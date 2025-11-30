use clap::Parser;
use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use renogy_rs::tui::{
    App, Event, EventHandler, Tab, VmClient, calculate_step_for_duration, draw, query_range,
};
use std::io::stdout;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const REFRESH_INTERVAL: Duration = Duration::from_secs(15);
const TICK_RATE: Duration = Duration::from_millis(250);
const MAX_HISTORY_SECS: u64 = 7 * 24 * 3600; // 7 days

#[derive(Parser)]
#[command(name = "renogy-tui")]
#[command(about = "TUI monitor for Renogy BMS batteries (VictoriaMetrics backend)")]
struct Args {
    /// VictoriaMetrics URL
    #[arg(long, default_value = "http://localhost:8428")]
    vm_url: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    eprintln!("Connecting to VictoriaMetrics at {}...", args.vm_url);
    let client =
        VmClient::new(&args.vm_url).map_err(|e| format!("Failed to create VM client: {}", e))?;

    eprintln!("Discovering batteries...");
    let batteries = match client.discover_batteries().await {
        Ok(b) => b,
        Err(e) => {
            eprintln!("Discovery error: {}", e);
            std::process::exit(1);
        }
    };

    if batteries.is_empty() {
        eprintln!("No batteries found in VictoriaMetrics!");
        eprintln!("Make sure renogy-bms-collector is running and has collected data.");
        std::process::exit(1);
    }

    eprintln!("Found {} battery(s): {:?}", batteries.len(), batteries);

    run_tui(client, batteries).await
}

async fn run_tui(
    client: VmClient,
    batteries: Vec<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(batteries.iter().map(|_| 0u8).collect());
    app.batteries = batteries.iter().map(|_| (0u8, None)).collect();

    let mut events = EventHandler::new(TICK_RATE);
    let mut last_refresh = Instant::now() - REFRESH_INTERVAL;
    let mut last_history_load: Option<Instant> = None;

    let result = run_event_loop(
        &mut terminal,
        &mut app,
        &mut events,
        &client,
        &mut last_refresh,
        &mut last_history_load,
        &batteries,
    )
    .await;

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

async fn run_event_loop(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    app: &mut App,
    events: &mut EventHandler,
    client: &VmClient,
    last_refresh: &mut Instant,
    last_history_load: &mut Option<Instant>,
    batteries: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
    while app.running {
        terminal.draw(|f| draw(f, app))?;

        let should_refresh = last_refresh.elapsed() >= REFRESH_INTERVAL;
        let should_load_history = last_history_load
            .map(|t| t.elapsed() >= REFRESH_INTERVAL)
            .unwrap_or(true);

        if let Some(event) = events.next().await {
            match event {
                Event::Quit => app.running = false,
                Event::Refresh => {
                    refresh_batteries(app, client, batteries).await;
                    *last_refresh = Instant::now();
                    if app.active_tab == Tab::Graphs {
                        load_history(app, client).await;
                        *last_history_load = Some(Instant::now());
                    }
                }
                Event::Tick if should_refresh => {
                    refresh_batteries(app, client, batteries).await;
                    *last_refresh = Instant::now();
                    if app.active_tab == Tab::Graphs && should_load_history {
                        load_history(app, client).await;
                        *last_history_load = Some(Instant::now());
                    }
                }
                Event::Key(key) => {
                    use crossterm::event::KeyCode;
                    match key.code {
                        KeyCode::Tab => {
                            app.next_tab();
                            if app.active_tab == Tab::Graphs && last_history_load.is_none() {
                                load_history(app, client).await;
                                *last_history_load = Some(Instant::now());
                            }
                        }
                        KeyCode::Up | KeyCode::Char('k') if app.active_tab == Tab::Overview => {
                            app.select_previous()
                        }
                        KeyCode::Down | KeyCode::Char('j') if app.active_tab == Tab::Overview => {
                            app.select_next()
                        }
                        KeyCode::Char('+') | KeyCode::Char('=')
                            if app.active_tab == Tab::Graphs =>
                        {
                            app.graph_view.zoom_in();
                            load_history(app, client).await;
                            *last_history_load = Some(Instant::now());
                        }
                        KeyCode::Char('-') if app.active_tab == Tab::Graphs => {
                            app.graph_view.zoom_out();
                            load_history(app, client).await;
                            *last_history_load = Some(Instant::now());
                        }
                        KeyCode::Left | KeyCode::Char('h') if app.active_tab == Tab::Graphs => {
                            let step = app.graph_view.zoom_window_secs() / 4;
                            app.graph_view.scroll_back(step, MAX_HISTORY_SECS);
                            load_history(app, client).await;
                            *last_history_load = Some(Instant::now());
                        }
                        KeyCode::Right | KeyCode::Char('l') if app.active_tab == Tab::Graphs => {
                            let step = app.graph_view.zoom_window_secs() / 4;
                            app.graph_view.scroll_forward(step);
                            load_history(app, client).await;
                            *last_history_load = Some(Instant::now());
                        }
                        KeyCode::Home | KeyCode::Char('g') if app.active_tab == Tab::Graphs => {
                            app.graph_view.jump_to_newest();
                            load_history(app, client).await;
                            *last_history_load = Some(Instant::now());
                        }
                        KeyCode::End | KeyCode::Char('G') if app.active_tab == Tab::Graphs => {
                            app.graph_view.jump_to_oldest(MAX_HISTORY_SECS);
                            load_history(app, client).await;
                            *last_history_load = Some(Instant::now());
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }

    Ok(())
}

async fn refresh_batteries(app: &mut App, client: &VmClient, batteries: &[String]) {
    app.refreshing = true;
    app.error = None;

    for (i, serial) in batteries.iter().enumerate() {
        match client.query_latest(serial).await {
            Ok(info) => {
                if i < app.batteries.len() {
                    app.batteries[i] = (i as u8, info);
                }
            }
            Err(e) => {
                app.error = Some(e);
            }
        }
    }

    app.last_update = Some(Instant::now());
    app.refreshing = false;
}

async fn load_history(app: &mut App, client: &VmClient) {
    let now_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let window_secs = app.graph_view.zoom_window_secs();
    let scroll_offset = app.graph_view.scroll_offset_secs;

    let end_secs = now_secs.saturating_sub(scroll_offset);
    let start_secs = end_secs.saturating_sub(window_secs);
    let step_secs = calculate_step_for_duration(window_secs);

    match query_range(client, start_secs, end_secs, step_secs).await {
        Ok(points) => {
            app.history.replace(points);
        }
        Err(e) => {
            app.error = Some(e);
        }
    }
}
