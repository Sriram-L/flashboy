use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, List, ListItem, Padding, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::{App, EditFocus, Screen};
use crate::store::{self, Verdict};
use crate::theme;

pub fn draw(frame: &mut Frame, app: &mut App) {
    let bg = Block::default().style(Style::default().bg(theme::PANEL).fg(theme::TEXT));
    frame.render_widget(bg, frame.area());

    match app.screen {
        Screen::Picker => draw_picker(frame, app),
        Screen::Main | Screen::ConfirmDelete | Screen::Help | Screen::Message | Screen::Edit => {
            draw_main(frame, app);
            match app.screen {
                Screen::Edit => draw_edit(frame, app),
                Screen::ConfirmDelete => draw_confirm(frame, app),
                Screen::Help => draw_help(frame),
                Screen::Message => draw_message(frame, app),
                _ => {}
            }
        }
    }
}

fn draw_picker(frame: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7),
            Constraint::Min(4),
            Constraint::Length(2),
        ])
        .split(frame.area());

    let hero = Paragraph::new(vec![
        Line::from(""),
        Line::from(Span::styled("  FLASHBOY", theme::title())),
        Line::from(Span::styled(
            "  compact C++ test harness",
            theme::muted(),
        )),
        Line::from(Span::styled(
            "  one .fbk bank per program  ·  g++-12  ·  5s TLE",
            theme::muted(),
        )),
    ])
    .block(outer("open a program"));
    frame.render_widget(hero, chunks[0]);

    let items: Vec<ListItem> = if app.cpp_files.is_empty() {
        vec![ListItem::new(Span::styled(
            "  no .cpp files in this directory — pass a path: flashboy main.cpp",
            theme::muted(),
        ))]
    } else {
        app.cpp_files
            .iter()
            .map(|p| {
                ListItem::new(Line::from(vec![
                    Span::styled("  ▸  ", Style::default().fg(theme::ACCENT)),
                    Span::styled(p.display().to_string(), theme::text()),
                ]))
            })
            .collect()
    };
    let list = List::new(items)
        .block(outer("sources"))
        .highlight_style(
            Style::default()
                .fg(theme::PANEL)
                .bg(theme::ACCENT)
                .add_modifier(Modifier::BOLD),
        );
    frame.render_stateful_widget(list, chunks[1], &mut app.picker_state);

    frame.render_widget(
        footer("j/k move  ·  enter open  ·  ? help  ·  q quit"),
        chunks[2],
    );
}

fn draw_main(frame: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(2),
        ])
        .split(frame.area());

    let title = match &app.cpp {
        Some(p) => {
            let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("program.cpp");
            let bank = app
                .bank_path
                .as_ref()
                .map(|b| b.file_name().and_then(|s| s.to_str()).unwrap_or(""))
                .unwrap_or("");
            format!(" FLASHBOY  ·  {name}  ·  {bank} ")
        }
        None => " FLASHBOY ".into(),
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(title, theme::title()),
            Span::styled(
                format!("  {}", app.status),
                theme::muted(),
            ),
        ]))
        .block(outer("")),
        chunks[0],
    );

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(34), Constraint::Percentage(66)])
        .split(chunks[1]);

    let items: Vec<ListItem> = app
        .bank
        .cases
        .iter()
        .map(|c| {
            let (badge, style) = match c.last.as_ref().map(|r| r.verdict) {
                Some(Verdict::Pass) => ("AC ", theme::pass()),
                Some(Verdict::Fail) => ("WA ", theme::fail()),
                Some(Verdict::Tle) => ("TLE", theme::tle()),
                Some(Verdict::Runtime) => ("RE ", theme::fail()),
                Some(Verdict::Ran) => ("OK ", theme::info()),
                None => (" · ", theme::muted()),
            };
            let time = c
                .last
                .as_ref()
                .map(|r| format!("  {}", store::format_duration(r.duration_ns)))
                .unwrap_or_default();
            ListItem::new(Line::from(vec![
                Span::styled(format!(" {badge} "), style),
                Span::styled(c.name.clone(), theme::text()),
                Span::styled(time, theme::muted()),
            ]))
        })
        .collect();

    let case_title = format!("cases ({})", app.bank.cases.len());
    let list = List::new(items)
        .block(outer(&case_title))
        .highlight_style(
            Style::default()
                .bg(theme::BORDER)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("│");
    frame.render_stateful_widget(list, body[0], &mut app.list_state);

    frame.render_widget(detail_widget(app), body[1]);

    let hint = if app.busy {
        "running… keys locked"
    } else {
        "n new  e edit  d delete  r run  R all  J/K scroll  ? help  q quit"
    };
    frame.render_widget(footer(hint), chunks[2]);
}

fn detail_widget(app: &App) -> Paragraph<'static> {
    let Some(c) = app.selected_case() else {
        return Paragraph::new(Span::styled(
            "\n  no case selected — press n to add one",
            theme::muted(),
        ))
        .block(outer("detail"));
    };

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(vec![
        Span::styled("  ", theme::muted()),
        Span::styled(c.name.clone(), theme::title()),
        Span::styled(format!("  #{}", c.id), theme::muted()),
    ]));

    if let Some(r) = &c.last {
        let vstyle = match r.verdict {
            Verdict::Pass => theme::pass(),
            Verdict::Fail => theme::fail(),
            Verdict::Tle => theme::tle(),
            Verdict::Runtime => theme::fail(),
            Verdict::Ran => theme::info(),
        };
        let extra = match r.verdict {
            Verdict::Tle => "  — exceeded 5.000 s",
            _ => "",
        };
        lines.push(Line::from(vec![
            Span::styled("  verdict  ", theme::muted()),
            Span::styled(r.verdict.label(), vstyle),
            Span::styled(
                format!("  {}", store::format_duration(r.duration_ns)),
                theme::text(),
            ),
            Span::styled(extra, theme::tle()),
        ]));
        if let Some(code) = r.exit_code {
            lines.push(Line::from(Span::styled(
                format!("  exit     {code}"),
                theme::muted(),
            )));
        }
    } else {
        lines.push(Line::from(Span::styled(
            "  not run yet",
            theme::muted(),
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled("  INPUT", Style::default().fg(theme::ACCENT).add_modifier(Modifier::BOLD))));
    push_body(&mut lines, &c.input);

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled("  EXPECTED", Style::default().fg(theme::ACCENT).add_modifier(Modifier::BOLD))));
    match &c.expected {
        Some(e) => push_body(&mut lines, e),
        None => lines.push(Line::from(Span::styled("  (none — output only)", theme::muted()))),
    }

    if let Some(r) = &c.last {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled("  OUTPUT", Style::default().fg(theme::ACCENT).add_modifier(Modifier::BOLD))));
        if r.verdict == Verdict::Tle {
            lines.push(Line::from(Span::styled(
                "  program killed after 5s (TLE)",
                theme::tle(),
            )));
        } else {
            push_body(&mut lines, &r.stdout);
        }
        if !r.stderr.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled("  STDERR", theme::fail())));
            push_body(&mut lines, &r.stderr);
        }
        if r.verdict == Verdict::Fail {
            if let Some(exp) = &c.expected {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled("  DIFF", theme::fail())));
                push_diff(&mut lines, exp, &r.stdout);
            }
        }
    }

    Paragraph::new(lines)
        .block(outer("detail"))
        .scroll((app.detail_scroll, 0))
        .wrap(Wrap { trim: false })
}

fn push_body(lines: &mut Vec<Line>, s: &str) {
    if s.is_empty() {
        lines.push(Line::from(Span::styled("  ∅", theme::muted())));
        return;
    }
    for (i, line) in s.lines().take(200).enumerate() {
        lines.push(Line::from(vec![
            Span::styled(format!("  {:>3} │ ", i + 1), theme::muted()),
            Span::raw(line.to_string()),
        ]));
    }
    if s.lines().count() > 200 {
        lines.push(Line::from(Span::styled("  … truncated", theme::muted())));
    }
}

fn push_diff(lines: &mut Vec<Line>, expected: &str, actual: &str) {
    let e = crate::runner::normalize(expected);
    let a = crate::runner::normalize(actual);
    let el: Vec<&str> = e.lines().collect();
    let al: Vec<&str> = a.lines().collect();
    let n = el.len().max(al.len()).min(80);
    for i in 0..n {
        let left = el.get(i).copied().unwrap_or("");
        let right = al.get(i).copied().unwrap_or("");
        if left == right {
            continue;
        }
        lines.push(Line::from(Span::styled(
            format!("  - {left}"),
            theme::fail(),
        )));
        lines.push(Line::from(Span::styled(
            format!("  + {right}"),
            theme::pass(),
        )));
    }
}

fn draw_edit(frame: &mut Frame, app: &mut App) {
    let area = centered(frame.area(), 86, 82);
    frame.render_widget(Clear, area);
    let block = outer("edit case  ·  tab fields  ·  ctrl-s save  ·  esc cancel");
    frame.render_widget(block, area);

    let inner = inset(area);
    let Some(edit) = app.edit.as_mut() else {
        return;
    };
    let cols = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Percentage(48),
            Constraint::Percentage(52),
        ])
        .split(inner);

    let name_active = matches!(edit.focus, EditFocus::Name);
    let name = Paragraph::new(format!(" {}", edit.name)).block(field("name", name_active));
    frame.render_widget(name, cols[0]);

    let input_active = matches!(edit.focus, EditFocus::Input);
    edit.input.set_block(field("input (stdin)", input_active));
    frame.render_widget(&edit.input, cols[1]);

    let exp_active = matches!(edit.focus, EditFocus::Expected);
    edit.expected
        .set_block(field("expected (optional)", exp_active));
    frame.render_widget(&edit.expected, cols[2]);
}

fn draw_confirm(frame: &mut Frame, app: &App) {
    let name = app
        .selected_case()
        .map(|c| c.name.as_str())
        .unwrap_or("case");
    modal(
        frame,
        "delete",
        vec![
            Line::from(""),
            Line::from(format!("  drop “{name}”?  y / n")),
            Line::from(""),
        ],
    );
}

fn draw_help(frame: &mut Frame) {
    modal(
        frame,
        "keys",
        vec![
            Line::from(""),
            Line::from("  n          new case"),
            Line::from("  e          edit case"),
            Line::from("  d          delete case"),
            Line::from("  r          run selected"),
            Line::from("  R / a      run all (compile once)"),
            Line::from("  j / k      move"),
            Line::from("  J / K      scroll detail"),
            Line::from("  esc        back to picker"),
            Line::from("  q          quit (auto-saves)"),
            Line::from(""),
            Line::from("  bank is postcard+zstd beside the .cpp as *.cpp.fbk"),
            Line::from("  TLE if a case exceeds 5 seconds"),
            Line::from(""),
        ],
    );
}

fn draw_message(frame: &mut Frame, app: &App) {
    let lines: Vec<Line> = std::iter::once(Line::from(""))
        .chain(app.message.lines().map(|l| Line::from(format!("  {l}"))))
        .chain(std::iter::once(Line::from("")))
        .chain(std::iter::once(Line::from(Span::styled(
            "  any key to close",
            theme::muted(),
        ))))
        .collect();
    modal(frame, "notice", lines);
}

fn modal(frame: &mut Frame, title: &str, lines: Vec<Line>) {
    let area = centered(frame.area(), 70, 18.max(lines.len() as u16 + 4).min(28));
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(lines)
            .block(outer(title))
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn outer(title: &str) -> Block<'_> {
    let mut b = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme::border())
        .style(Style::default().bg(theme::PANEL).fg(theme::TEXT))
        .padding(Padding::new(1, 1, 0, 0));
    if !title.is_empty() {
        b = b.title(Span::styled(format!(" {title} "), theme::muted()));
    }
    b
}

fn field(title: &str, active: bool) -> Block<'_> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(if active {
            theme::border_active()
        } else {
            theme::border()
        })
        .title(Span::styled(
            format!(" {title} "),
            if active { theme::title() } else { theme::muted() },
        ))
}

fn footer(text: &str) -> Paragraph<'_> {
    Paragraph::new(Span::styled(format!("  {text}"), theme::muted()))
}

fn centered(area: Rect, pct_x: u16, height: u16) -> Rect {
    let h = height.min(area.height.saturating_sub(2));
    let w = (area.width.saturating_mul(pct_x) / 100).min(area.width.saturating_sub(4));
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    Rect {
        x,
        y,
        width: w,
        height: h,
    }
}

fn inset(area: Rect) -> Rect {
    Rect {
        x: area.x + 1,
        y: area.y + 1,
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    }
}
