use std::{
    io,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread,
    time::Duration,
};

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};

use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Layout},
    style::{Color, Style},
    symbols,
    text::{Line, Span},
    widgets::{Axis, Block, Borders, Chart, Dataset, GraphType},
};

mod collector;
mod state;

use state::*;

const DEFAULT_INTERVAL_MS: u64 = 200;
const MIN_INTERVAL_MS: u64 = 10;
const MAX_INTERVAL_MS: u64 = 2000;
const RENDER_INTERVAL: Duration = Duration::from_millis(50);

const KB: f64 = 1024.0;
const MB: f64 = KB * 1024.0;
const GB: f64 = MB * 1024.0;

/// Round up to a "nice" number (1, 2, 5 × 10^n) in the appropriate byte unit.
fn nice_ceil(val: f64) -> f64 {
    if val <= 0.0 {
        return KB;
    }
    let unit = [GB, MB, KB, 1.0].into_iter().find(|&u| val >= u).unwrap();
    let scaled = val / unit;
    let exp = scaled.log10().floor();
    let frac = scaled / 10_f64.powf(exp);
    let nice = match () {
        _ if frac <= 1.0 => 1.0,
        _ if frac <= 2.0 => 2.0,
        _ if frac <= 5.0 => 5.0,
        _ => 10.0,
    };
    nice * 10_f64.powf(exp) * unit
}

fn format_bytes(bytes: f64) -> String {
    let abs = bytes.abs();
    if abs >= GB {
        format!("{:.1} GB/s", bytes / GB)
    } else if abs >= MB {
        format!("{:.1} MB/s", bytes / MB)
    } else if abs >= KB {
        format!("{:.1} KB/s", bytes / KB)
    } else {
        format!("{:.0} B/s", bytes)
    }
}

fn build_points(series: &Series, graph_width: usize) -> Vec<(f64, f64)> {
    let n = series.samples.len();
    let skip = n.saturating_sub(graph_width);
    let offset = graph_width.saturating_sub(n);
    series
        .samples
        .iter()
        .skip(skip)
        .enumerate()
        .map(|(i, &v)| ((i + offset) as f64, v))
        .collect()
}

fn draw_chart(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    title: &str,
    points: &[(f64, f64)],
    color: Color,
    graph_width: usize,
) {
    let y_max = nice_ceil(points.iter().map(|(_, y)| *y).fold(0.0_f64, f64::max));

    let dataset = Dataset::default()
        .marker(symbols::Marker::Braille)
        .graph_type(GraphType::Bar)
        .style(Style::default().fg(color))
        .data(points);

    let x_axis = Axis::default()
        .style(Style::default().fg(Color::DarkGray))
        .bounds([0.0, graph_width as f64]);

    let y_axis = Axis::default()
        .style(Style::default().fg(Color::DarkGray))
        .bounds([0.0, y_max])
        .labels(vec![
            Line::from("0"),
            Line::from(format_bytes(y_max / 2.0)),
            Line::from(format_bytes(y_max)),
        ]);

    let chart = Chart::new(vec![dataset])
        .block(
            Block::default()
                .title(title.to_string())
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .x_axis(x_axis)
        .y_axis(y_axis);

    frame.render_widget(chart, area);
}

fn draw(frame: &mut Frame, state: &State, interval_ms: u64) {
    let chunks = Layout::vertical([
        Constraint::Min(5),
        Constraint::Min(5),
        Constraint::Length(1),
    ])
    .split(frame.area());

    let graph_width = chunks[0].width.saturating_sub(10) as usize;

    let trans_points = build_points(&state.trans, graph_width);
    let retrans_points = build_points(&state.retrans, graph_width);

    draw_chart(
        frame,
        chunks[0],
        &format!(" TCP tx: {} ", format_bytes(state.trans.current_rate())),
        &trans_points,
        Color::Green,
        graph_width,
    );

    draw_chart(
        frame,
        chunks[1],
        &format!(" TCP retx: {} ", format_bytes(state.retrans.current_rate())),
        &retrans_points,
        Color::Red,
        graph_width,
    );

    let footer = Line::from(vec![
        Span::styled(
            format!(" interval: {}ms ", interval_ms),
            Style::default().fg(Color::Green),
        ),
        Span::raw("| "),
        Span::styled("+/- ", Style::default().fg(Color::White)),
        Span::raw("adjust interval  "),
        Span::styled("q ", Style::default().fg(Color::White)),
        Span::raw("quit"),
    ]);
    frame.render_widget(footer, chunks[2]);
}

fn restore_terminal() {
    let _ = terminal::disable_raw_mode();
    let _ = crossterm::execute!(io::stdout(), LeaveAlternateScreen);
}

fn main() -> anyhow::Result<()> {
    // Bump memlock rlimit
    let rlim = libc::rlimit {
        rlim_cur: libc::RLIM_INFINITY,
        rlim_max: libc::RLIM_INFINITY,
    };
    let ret = unsafe { libc::setrlimit(libc::RLIMIT_MEMLOCK, &rlim) };
    if ret != 0 {
        eprintln!("warning: failed to remove memlock rlimit");
    }

    let state = Arc::new(Mutex::new(State::default()));
    let interval_ms = Arc::new(AtomicU64::new(DEFAULT_INTERVAL_MS));
    let quit = Arc::new(AtomicBool::new(false));

    let collector = {
        let state = Arc::clone(&state);
        let interval_ms = Arc::clone(&interval_ms);
        let quit = Arc::clone(&quit);
        thread::spawn(move || collector::collector_thread(state, interval_ms, quit))
    };

    // Setup terminal
    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    crossterm::execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        restore_terminal();
        original_hook(panic_info);
    }));

    // Main loop: render + handle input
    let result: anyhow::Result<()> = 'main: loop {
        while event::poll(Duration::ZERO)? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                let ms = interval_ms.load(Ordering::Relaxed);
                match key.code {
                    KeyCode::Char('q') => break 'main Ok(()),
                    KeyCode::Char('-') => {
                        let step = if ms <= 100 { 10 } else { 100 };
                        interval_ms.store(
                            ms.saturating_sub(step).max(MIN_INTERVAL_MS),
                            Ordering::Relaxed,
                        );
                    }
                    KeyCode::Char('+') | KeyCode::Char('=') => {
                        let step = if ms < 100 { 10 } else { 100 };
                        interval_ms.store((ms + step).min(MAX_INTERVAL_MS), Ordering::Relaxed);
                    }
                    _ => {}
                }
            }
        }

        {
            let s = state.lock().unwrap();
            let ms = interval_ms.load(Ordering::Relaxed);
            terminal.draw(|frame| draw(frame, &s, ms))?;
        }

        thread::sleep(RENDER_INTERVAL);
    };

    quit.store(true, Ordering::Relaxed);
    let _ = collector.join();
    restore_terminal();
    result
}
