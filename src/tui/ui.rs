use crate::query::BatteryInfo;
use crate::tui::app::App;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, List, ListItem, Paragraph},
};

pub fn draw(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Min(10),
            Constraint::Length(1),
        ])
        .split(frame.area());

    draw_rollup(frame, app, chunks[0]);
    draw_main_area(frame, app, chunks[1]);
    draw_status_bar(frame, app, chunks[2]);
}

fn draw_rollup(frame: &mut Frame, app: &App, area: Rect) {
    let rollup = app.rollup();

    let temp_str = match (rollup.min_temperature, rollup.max_temperature) {
        (Some(min), Some(max)) => format!("{:.1}°C - {:.1}°C", min, max),
        _ => "N/A".to_string(),
    };

    let current_sign = if rollup.total_current >= 0.0 { "+" } else { "" };

    let text = vec![
        Line::from(vec![
            Span::styled("Total Current: ", Style::default()),
            Span::styled(
                format!("{}{:.1}A", current_sign, rollup.total_current),
                Style::default().fg(if rollup.total_current >= 0.0 {
                    Color::Green
                } else {
                    Color::Yellow
                }),
            ),
            Span::raw("    "),
            Span::styled("Avg SOC: ", Style::default()),
            Span::styled(
                format!("{:.0}%", rollup.average_soc),
                Style::default().fg(soc_color(rollup.average_soc)),
            ),
            Span::raw("    "),
            Span::styled("Capacity: ", Style::default()),
            Span::styled(
                format!(
                    "{:.0}/{:.0} Ah",
                    rollup.total_remaining_ah, rollup.total_capacity_ah
                ),
                Style::default(),
            ),
        ]),
        Line::from(vec![
            Span::styled("Temperature Range: ", Style::default()),
            Span::styled(temp_str, Style::default().fg(Color::Cyan)),
        ]),
    ];

    let title = format!(
        " System Roll Up ({}/{} responding) ",
        rollup.responding_count, rollup.battery_count
    );

    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .title_style(Style::default().add_modifier(Modifier::BOLD));

    let paragraph = Paragraph::new(text).block(block);
    frame.render_widget(paragraph, area);
}

fn draw_main_area(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(20), Constraint::Min(40)])
        .split(area);

    draw_battery_list(frame, app, chunks[0]);
    draw_battery_detail(frame, app, chunks[1]);
}

fn draw_battery_list(frame: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = app
        .batteries
        .iter()
        .enumerate()
        .map(|(i, (addr, info))| {
            let voltage_str = info
                .as_ref()
                .map(|b| format!("{:.1}V", b.module_voltage))
                .unwrap_or_else(|| "---".to_string());

            let content = format!("0x{:02X} {}", addr, voltage_str);

            let style = if i == app.selected {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else if info.is_some() {
                Style::default().fg(Color::White)
            } else {
                Style::default().fg(Color::DarkGray)
            };

            let prefix = if i == app.selected { "> " } else { "  " };
            ListItem::new(format!("{}{}", prefix, content)).style(style)
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Batteries ")
            .title_style(Style::default().add_modifier(Modifier::BOLD)),
    );

    frame.render_widget(list, area);
}

fn draw_battery_detail(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Battery Details ")
        .title_style(Style::default().add_modifier(Modifier::BOLD));

    let Some(battery) = app.selected_battery() else {
        let addr = app
            .batteries
            .get(app.selected)
            .map(|(a, _)| *a)
            .unwrap_or(0);
        let text = Paragraph::new(format!("No data for 0x{:02X}", addr)).block(block);
        frame.render_widget(text, area);
        return;
    };

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Length(3),
            Constraint::Length(4),
            Constraint::Min(6),
            Constraint::Length(2),
        ])
        .split(inner);

    draw_battery_info(frame, battery, chunks[0]);
    draw_soc_gauge(frame, battery, chunks[1]);
    draw_electrical(frame, battery, chunks[2]);
    draw_cell_voltages(frame, battery, chunks[3]);
    draw_temperatures(frame, battery, chunks[4]);
}

fn draw_battery_info(frame: &mut Frame, battery: &BatteryInfo, area: Rect) {
    let text = vec![
        Line::from(vec![
            Span::styled("Model: ", Style::default().fg(Color::DarkGray)),
            Span::raw(&battery.model),
        ]),
        Line::from(vec![
            Span::styled("Serial: ", Style::default().fg(Color::DarkGray)),
            Span::raw(&battery.serial),
        ]),
        Line::from(vec![
            Span::styled("Software: ", Style::default().fg(Color::DarkGray)),
            Span::raw(&battery.software_version),
            Span::raw("  "),
            Span::styled("Mfr: ", Style::default().fg(Color::DarkGray)),
            Span::raw(&battery.manufacturer),
        ]),
    ];
    frame.render_widget(Paragraph::new(text), area);
}

fn draw_soc_gauge(frame: &mut Frame, battery: &BatteryInfo, area: Rect) {
    let soc = battery.soc_percent.clamp(0.0, 100.0);
    let gauge = Gauge::default()
        .block(Block::default().title("SOC"))
        .gauge_style(Style::default().fg(soc_color(soc)))
        .percent(soc as u16)
        .label(format!("{:.1}%", soc));
    frame.render_widget(gauge, area);
}

fn draw_electrical(frame: &mut Frame, battery: &BatteryInfo, area: Rect) {
    let current_sign = if battery.current >= 0.0 { "+" } else { "" };
    let current_color = if battery.current >= 0.0 {
        Color::Green
    } else {
        Color::Yellow
    };

    let text = vec![
        Line::from(vec![
            Span::styled("Voltage: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{:.2}V", battery.module_voltage),
                Style::default().fg(Color::Cyan),
            ),
            Span::raw("   "),
            Span::styled("Current: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{}{:.2}A", current_sign, battery.current),
                Style::default().fg(current_color),
            ),
        ]),
        Line::from(vec![
            Span::styled("Capacity: ", Style::default().fg(Color::DarkGray)),
            Span::raw(format!(
                "{:.1}/{:.1} Ah",
                battery.remaining_capacity, battery.total_capacity
            )),
            Span::raw("   "),
            Span::styled("Cycles: ", Style::default().fg(Color::DarkGray)),
            Span::raw(format!("{}", battery.cycle_count)),
        ]),
    ];
    frame.render_widget(Paragraph::new(text), area);
}

fn draw_cell_voltages(frame: &mut Frame, battery: &BatteryInfo, area: Rect) {
    if battery.cell_voltages.is_empty() {
        return;
    }

    let min_v = battery
        .cell_voltages
        .iter()
        .cloned()
        .fold(f32::MAX, f32::min);
    let max_v = battery
        .cell_voltages
        .iter()
        .cloned()
        .fold(f32::MIN, f32::max);
    let delta = max_v - min_v;

    let cols = 4;
    let mut lines: Vec<Line> = vec![Line::from(Span::styled(
        "Cell Voltages:",
        Style::default().add_modifier(Modifier::BOLD),
    ))];

    for chunk in battery.cell_voltages.chunks(cols) {
        let spans: Vec<Span> = chunk
            .iter()
            .map(|&v| {
                let color = if (v - min_v).abs() < 0.001 && delta > 0.01 {
                    Color::Red
                } else if (v - max_v).abs() < 0.001 && delta > 0.01 {
                    Color::Green
                } else {
                    Color::White
                };
                Span::styled(format!("{:>6.3}V ", v), Style::default().fg(color))
            })
            .collect();
        lines.push(Line::from(spans));
    }

    lines.push(Line::from(vec![
        Span::styled("Min: ", Style::default().fg(Color::DarkGray)),
        Span::styled(format!("{:.3}", min_v), Style::default().fg(Color::Red)),
        Span::raw("  "),
        Span::styled("Max: ", Style::default().fg(Color::DarkGray)),
        Span::styled(format!("{:.3}", max_v), Style::default().fg(Color::Green)),
        Span::raw("  "),
        Span::styled("Δ: ", Style::default().fg(Color::DarkGray)),
        Span::raw(format!("{:.3}V", delta)),
    ]));

    frame.render_widget(Paragraph::new(lines), area);
}

fn draw_temperatures(frame: &mut Frame, battery: &BatteryInfo, area: Rect) {
    if battery.cell_temperatures.is_empty() {
        return;
    }

    let min_t = battery
        .cell_temperatures
        .iter()
        .cloned()
        .fold(f32::MAX, f32::min);
    let max_t = battery
        .cell_temperatures
        .iter()
        .cloned()
        .fold(f32::MIN, f32::max);

    let temps: String = battery
        .cell_temperatures
        .iter()
        .map(|t| format!("{:.1}°C", t))
        .collect::<Vec<_>>()
        .join("  ");

    let line = Line::from(vec![
        Span::styled("Temps: ", Style::default().fg(Color::DarkGray)),
        Span::raw(temps),
        Span::raw("  ("),
        Span::styled(format!("{:.1}", min_t), Style::default().fg(Color::Cyan)),
        Span::raw(" - "),
        Span::styled(format!("{:.1}°C", max_t), Style::default().fg(Color::Cyan)),
        Span::raw(")"),
    ]);

    frame.render_widget(Paragraph::new(vec![line]), area);
}

fn draw_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let last_update = app
        .last_update
        .map(|t| {
            let secs = t.elapsed().as_secs();
            if secs < 60 {
                format!("{}s ago", secs)
            } else {
                format!("{}m ago", secs / 60)
            }
        })
        .unwrap_or_else(|| "never".to_string());

    let status = if app.refreshing {
        Span::styled("Refreshing...", Style::default().fg(Color::Yellow))
    } else if app.error.is_some() {
        Span::styled("Error", Style::default().fg(Color::Red))
    } else {
        Span::styled("Connected", Style::default().fg(Color::Green))
    };

    let line = Line::from(vec![
        Span::styled(" [q]", Style::default().fg(Color::DarkGray)),
        Span::raw("uit  "),
        Span::styled("[↑↓/jk]", Style::default().fg(Color::DarkGray)),
        Span::raw("select  "),
        Span::styled("[r]", Style::default().fg(Color::DarkGray)),
        Span::raw("efresh"),
        Span::raw("  │  "),
        Span::raw(format!("Updated: {} ", last_update)),
        Span::raw("  │  "),
        status,
    ]);

    frame.render_widget(Paragraph::new(line), area);
}

fn soc_color(soc: f32) -> Color {
    if soc >= 50.0 {
        Color::Green
    } else if soc >= 20.0 {
        Color::Yellow
    } else {
        Color::Red
    }
}
