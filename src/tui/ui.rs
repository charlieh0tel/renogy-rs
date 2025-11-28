use crate::tui::app::App;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
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
        (Some(min), Some(max)) => format!("{:.1}-{:.1}°C", min, max),
        _ => "N/A".to_string(),
    };

    let current_sign = if rollup.total_current >= 0.0 { "+" } else { "" };

    let line1 = Line::from(vec![
        Span::styled("Current: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{}{:.1}A", current_sign, rollup.total_current),
            Style::default().fg(if rollup.total_current >= 0.0 {
                Color::Green
            } else {
                Color::Yellow
            }),
        ),
        Span::raw("  "),
        Span::styled("Temp: ", Style::default().fg(Color::DarkGray)),
        Span::styled(temp_str, Style::default().fg(Color::Cyan)),
    ]);

    // Capacity bar
    let soc = rollup.average_soc.clamp(0.0, 100.0);
    let bar_width = 40;
    let filled = ((soc / 100.0) * bar_width as f32) as usize;
    let empty = bar_width - filled;
    let bar = format!("{}{}", "█".repeat(filled), "░".repeat(empty));

    let line2 = Line::from(vec![
        Span::styled("Capacity: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{:5.1}% ", soc),
            Style::default().fg(soc_color(soc)),
        ),
        Span::styled(bar, Style::default().fg(soc_color(soc))),
        Span::raw(format!(
            " {:.0}/{:.0}Ah",
            rollup.total_remaining_ah, rollup.total_capacity_ah
        )),
    ]);

    let title = format!(
        " Roll Up ({}/{}) ",
        rollup.responding_count, rollup.battery_count
    );

    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .title_style(Style::default().add_modifier(Modifier::BOLD));

    let paragraph = Paragraph::new(vec![line1, line2]).block(block);
    frame.render_widget(paragraph, area);
}

fn draw_main_area(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(18), Constraint::Min(40)])
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

            let prefix = if i == app.selected { ">" } else { " " };
            let content = format!("{} 0x{:02X} {}", prefix, addr, voltage_str);

            let style = if i == app.selected {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else if info.is_some() {
                Style::default().fg(Color::White)
            } else {
                Style::default().fg(Color::DarkGray)
            };

            ListItem::new(content).style(style)
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

    let mut lines: Vec<Line> = Vec::new();

    // Line 1: Model and serial
    lines.push(Line::from(vec![
        Span::styled(
            &battery.model,
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled("SN:", Style::default().fg(Color::DarkGray)),
        Span::raw(&battery.serial),
        Span::raw("  "),
        Span::styled("v", Style::default().fg(Color::DarkGray)),
        Span::raw(&battery.software_version),
    ]));

    // Line 2: Electrical stats
    let current_sign = if battery.current >= 0.0 { "+" } else { "" };
    let current_color = if battery.current >= 0.0 {
        Color::Green
    } else {
        Color::Yellow
    };

    lines.push(Line::from(vec![
        Span::styled(
            format!("{:.2}V", battery.module_voltage),
            Style::default().fg(Color::Cyan),
        ),
        Span::raw("  "),
        Span::styled(
            format!("{}{:.2}A", current_sign, battery.current),
            Style::default().fg(current_color),
        ),
        Span::raw("  "),
        Span::raw(format!(
            "{:.1}/{:.1}Ah",
            battery.remaining_capacity, battery.total_capacity
        )),
        Span::raw("  "),
        Span::styled(
            format!("{} cyc", battery.cycle_count),
            Style::default().fg(Color::DarkGray),
        ),
    ]));

    // Line 3: SOC bar
    let soc = battery.soc_percent.clamp(0.0, 100.0);
    let bar_width = 30;
    let filled = ((soc / 100.0) * bar_width as f32) as usize;
    let empty = bar_width - filled;
    let bar = format!("{}{}", "█".repeat(filled), "░".repeat(empty));

    lines.push(Line::from(vec![
        Span::styled("SOC: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{:5.1}% ", soc),
            Style::default().fg(soc_color(soc)),
        ),
        Span::styled(bar, Style::default().fg(soc_color(soc))),
    ]));

    // Blank line
    lines.push(Line::from(""));

    // Cell voltages
    if !battery.cell_voltages.is_empty() {
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

        lines.push(Line::from(Span::styled(
            format!(
                "Cells: Min:{:.3} Max:{:.3} Δ:{:.0}mV",
                min_v,
                max_v,
                delta * 1000.0
            ),
            Style::default().fg(Color::DarkGray),
        )));

        for chunk in battery.cell_voltages.chunks(4) {
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
                    Span::styled(format!(" {:>6.3}", v), Style::default().fg(color))
                })
                .collect();
            lines.push(Line::from(spans));
        }
    }

    // Temperatures
    if !battery.cell_temperatures.is_empty() {
        lines.push(Line::from("")); // blank line

        let temps: Vec<Span> =
            std::iter::once(Span::styled("Temp: ", Style::default().fg(Color::DarkGray)))
                .chain(battery.cell_temperatures.iter().map(|t| {
                    Span::styled(format!("{:.1}°C ", t), Style::default().fg(Color::Cyan))
                }))
                .collect();

        lines.push(Line::from(temps));
    }

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

fn draw_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let last_update = app
        .last_update
        .map(|t| {
            let secs = t.elapsed().as_secs();
            if secs < 60 {
                format!("{}s", secs)
            } else {
                format!("{}m", secs / 60)
            }
        })
        .unwrap_or_else(|| "-".to_string());

    let status = if app.refreshing {
        Span::styled("Refreshing", Style::default().fg(Color::Yellow))
    } else if app.error.is_some() {
        Span::styled("Error", Style::default().fg(Color::Red))
    } else {
        Span::styled("OK", Style::default().fg(Color::Green))
    };

    let line = Line::from(vec![
        Span::styled(" q", Style::default().fg(Color::DarkGray)),
        Span::raw(":quit "),
        Span::styled("↑↓", Style::default().fg(Color::DarkGray)),
        Span::raw(":sel "),
        Span::styled("r", Style::default().fg(Color::DarkGray)),
        Span::raw(":refresh │ "),
        status,
        Span::raw(format!(" │ {}", last_update)),
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
