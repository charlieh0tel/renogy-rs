use crate::tui::app::App;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

const LABEL: Style = Style::new().add_modifier(Modifier::DIM);
const BOLD: Style = Style::new().add_modifier(Modifier::BOLD);

fn color_current(amps: f32) -> Color {
    if amps >= 0.0 {
        Color::Green
    } else {
        Color::Yellow
    }
}

fn color_soc(soc: f32) -> Color {
    if soc >= 50.0 {
        Color::Green
    } else if soc >= 20.0 {
        Color::Yellow
    } else {
        Color::Red
    }
}

pub fn draw(frame: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7),
            Constraint::Min(14),
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
        (Some(min), Some(max)) => format!("{:.1}-{:.1}C", min, max),
        _ => "N/A".to_string(),
    };

    let current_sign = if rollup.total_current >= 0.0 { "+" } else { "" };

    let soc = rollup.average_soc.clamp(0.0, 100.0);
    let bar_width = 40;
    let filled = ((soc / 100.0) * bar_width as f32) as usize;
    let empty = bar_width - filled;
    let bar = format!("{}{}", "█".repeat(filled), "░".repeat(empty));

    let mut lines = Vec::new();

    // Line 1: Current and capacity
    lines.push(Line::from(vec![
        Span::styled("Current: ", LABEL),
        Span::styled(
            format!("{}{:.1}A", current_sign, rollup.total_current),
            Style::default().fg(color_current(rollup.total_current)),
        ),
        Span::raw("    "),
        Span::styled("Capacity: ", LABEL),
        Span::raw(format!(
            "{:.0}/{:.0}Ah",
            rollup.total_remaining_ah, rollup.total_capacity_ah
        )),
        Span::raw("    "),
        Span::styled("Temp: ", LABEL),
        Span::styled(temp_str, Style::default().fg(Color::Cyan)),
    ]));

    // Line 2: Empty for spacing
    lines.push(Line::from(""));

    // Line 3: SOC bar
    lines.push(Line::from(vec![
        Span::styled("SOC: ", LABEL),
        Span::styled(
            format!("{:5.1}% ", soc),
            Style::default().fg(color_soc(soc)),
        ),
        Span::styled(bar, Style::default().fg(color_soc(soc))),
    ]));

    let title = format!(
        " Roll Up ({}/{}) ",
        rollup.responding_count, rollup.battery_count
    );

    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .title_style(BOLD);

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

fn draw_main_area(frame: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(18), Constraint::Min(40)])
        .split(area);

    draw_battery_list(frame, app, chunks[0]);
    draw_battery_detail(frame, app, chunks[1]);
}

fn draw_battery_list(frame: &mut Frame, app: &mut App, area: Rect) {
    let items: Vec<ListItem> = app
        .batteries
        .iter()
        .map(|(addr, info)| {
            let voltage_str = info
                .as_ref()
                .map(|b| format!("{:.1}V", b.module_voltage))
                .unwrap_or_else(|| "---".to_string());

            let content = format!("0x{:02X} {}", addr, voltage_str);

            let style = if info.is_some() {
                Style::default()
            } else {
                LABEL
            };

            ListItem::new(content).style(style)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Batteries ")
                .title_style(BOLD),
        )
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");

    frame.render_stateful_widget(list, area, &mut app.list_state);
}

fn draw_battery_detail(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Battery Details ")
        .title_style(BOLD);

    let Some(battery) = app.selected_battery() else {
        let addr = app
            .batteries
            .get(app.selected())
            .map(|(a, _)| *a)
            .unwrap_or(0);
        let text = Paragraph::new(format!("No data for 0x{:02X}", addr)).block(block);
        frame.render_widget(text, area);
        return;
    };

    let mut lines: Vec<Line> = Vec::new();

    // Line 1: Model and serial
    lines.push(Line::from(vec![
        Span::styled(&battery.model, BOLD),
        Span::raw("  "),
        Span::styled("SN:", LABEL),
        Span::raw(&battery.serial),
        Span::raw("  "),
        Span::raw(&battery.software_version),
    ]));

    // Blank line
    lines.push(Line::from(""));

    // Electrical stats
    let current_sign = if battery.current >= 0.0 { "+" } else { "" };

    lines.push(Line::from(vec![
        Span::styled("Voltage: ", LABEL),
        Span::styled(
            format!("{:.2}V", battery.module_voltage),
            Style::default().fg(Color::Cyan),
        ),
        Span::raw("    "),
        Span::styled("Current: ", LABEL),
        Span::styled(
            format!("{}{:.2}A", current_sign, battery.current),
            Style::default().fg(color_current(battery.current)),
        ),
        Span::raw("    "),
        Span::styled("Cycles: ", LABEL),
        Span::raw(format!("{}", battery.cycle_count)),
    ]));

    lines.push(Line::from(vec![
        Span::styled("Capacity: ", LABEL),
        Span::raw(format!(
            "{:.1}/{:.1}Ah",
            battery.remaining_capacity, battery.total_capacity
        )),
    ]));

    // Blank line
    lines.push(Line::from(""));

    // SOC bar - wider
    let soc = battery.soc_percent.clamp(0.0, 100.0);
    let bar_width = 40;
    let filled = ((soc / 100.0) * bar_width as f32) as usize;
    let empty = bar_width - filled;
    let bar = format!("{}{}", "█".repeat(filled), "░".repeat(empty));

    lines.push(Line::from(vec![
        Span::styled("SOC: ", LABEL),
        Span::styled(
            format!("{:5.1}% ", soc),
            Style::default().fg(color_soc(soc)),
        ),
        Span::styled(bar, Style::default().fg(color_soc(soc))),
    ]));

    // Blank line
    lines.push(Line::from(""));

    // Temperatures
    if !battery.cell_temperatures.is_empty() {
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
        lines.push(Line::from(vec![
            Span::styled("Temp: ", LABEL),
            Span::styled(
                format!("{:.1}-{:.1}C", min_t, max_t),
                Style::default().fg(Color::Cyan),
            ),
            Span::styled(
                format!(" ({} sensors)", battery.cell_temperatures.len()),
                LABEL,
            ),
        ]));
    } else {
        lines.push(Line::from(vec![
            Span::styled("Temp: ", LABEL),
            Span::raw("(no data)"),
        ]));
    }

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

        lines.push(Line::from(vec![
            Span::styled(format!("Cells[{}]: ", battery.cell_voltages.len()), LABEL),
            Span::styled(format!("{:.3}", min_v), Style::default().fg(Color::Red)),
            Span::raw("-"),
            Span::styled(format!("{:.3}V", max_v), Style::default().fg(Color::Green)),
            Span::styled(format!(" Δ{:3.0}mV", delta * 1000.0), LABEL),
        ]));

        // Cell voltage grid - 4 per row with row label
        for (i, chunk) in battery.cell_voltages.chunks(4).enumerate() {
            let row_start = i * 4 + 1;
            let mut spans = vec![Span::styled(format!(" {:>2}: ", row_start), LABEL)];
            for &v in chunk {
                let style = if delta > 0.005 && (v - min_v).abs() < 0.001 {
                    Style::default().fg(Color::Red)
                } else if delta > 0.005 && (v - max_v).abs() < 0.001 {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default()
                };
                spans.push(Span::styled(format!("{:.3}  ", v), style));
            }
            lines.push(Line::from(spans));
        }
    } else {
        lines.push(Line::from(vec![
            Span::styled(format!("Cells[{}]: ", battery.cell_count), LABEL),
            Span::raw("(no voltage data)"),
        ]));
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
        Span::styled(" q", LABEL),
        Span::raw(":quit "),
        Span::styled("jk", LABEL),
        Span::raw(":sel "),
        Span::styled("r", LABEL),
        Span::raw(":refresh | "),
        status,
        Span::raw(format!(" | {}", last_update)),
    ]);

    frame.render_widget(Paragraph::new(line), area);
}
