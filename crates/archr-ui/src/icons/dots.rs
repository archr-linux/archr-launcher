//! NanoSVG drops Lucide's near-zero round-capped strokes. Normalize only those
//! dots in memory; keep the upstream assets and all ordinary geometry intact.
use quick_xml::{
    events::{BytesStart, Event},
    Reader, Writer,
};

fn attr(tag: &BytesStart<'_>, key: &str) -> Option<String> {
    tag.try_get_attribute(key)
        .ok()
        .flatten()
        .and_then(|a| a.unescape_value().ok().map(|v| v.into_owned()))
}

fn element(tag: &BytesStart<'_>, name: &'static str, omitted: &[&str]) -> BytesStart<'static> {
    BytesStart::new(name).with_attributes(
        tag.attributes()
            .filter_map(Result::ok)
            .filter(|a| !omitted.iter().any(|key| a.key.as_ref() == key.as_bytes())),
    )
}

fn number(input: &mut &str) -> Option<f32> {
    *input = input.trim_start_matches(|c: char| c.is_ascii_whitespace() || c == ',');
    let bytes = input.as_bytes();
    let mut end = usize::from(bytes.first().is_some_and(|b| matches!(b, b'+' | b'-')));
    while bytes
        .get(end)
        .is_some_and(|b| b.is_ascii_digit() || *b == b'.')
    {
        end += 1;
    }
    if bytes.get(end).is_some_and(|b| matches!(b, b'e' | b'E')) {
        end += 1;
        if bytes.get(end).is_some_and(|b| matches!(b, b'+' | b'-')) {
            end += 1;
        }
        while bytes.get(end).is_some_and(u8::is_ascii_digit) {
            end += 1;
        }
    }
    let value: f32 = input[..end].parse().ok()?;
    *input = &input[end..];
    value.is_finite().then_some(value)
}

fn segment(path: &str, previous: Option<(f32, f32)>) -> Option<((f32, f32), (f32, f32))> {
    let relative = path.starts_with('m');
    let mut data = &path[1..];
    let (mut x, mut y) = (number(&mut data)?, number(&mut data)?);
    if relative {
        let (px, py) = previous?;
        x += px;
        y += py;
    }
    data = data.trim_start();
    let mut command = data.chars().next()?;
    if command.is_ascii_alphabetic() {
        data = &data[1..];
    } else {
        command = if relative { 'l' } else { 'L' };
    }
    let n = number(&mut data)?;
    let end = match command {
        'h' => (x + n, y),
        'v' => (x, y + n),
        'H' => (n, y),
        'V' => (x, n),
        'l' => (x + n, y + number(&mut data)?),
        'L' => (n, number(&mut data)?),
        _ => return None,
    };
    data.trim().is_empty().then_some(((x, y), end))
}

fn tiny((x, y): (f32, f32), (ex, ey): (f32, f32)) -> bool {
    (ex - x).hypot(ey - y) <= 0.0101
}

fn path_dots(path: &str) -> (String, Vec<(f32, f32)>) {
    let mut kept = String::new();
    let mut dots = Vec::new();
    let mut previous = Some((0., 0.));
    let starts: Vec<_> = path
        .match_indices(['M', 'm'])
        .map(|(i, _)| i)
        .chain([path.len()])
        .collect();
    kept.push_str(&path[..starts[0]]);
    for pair in starts.windows(2) {
        let part = &path[pair[0]..pair[1]];
        let origin = previous;
        if let Some((point, end)) = segment(part, previous) {
            previous = Some(end);
            if tiny(point, end) {
                dots.push(point);
                continue;
            }
        } else {
            previous = None;
        }
        // Removing a dot must not change the origin of a following relative move.
        if let Some((x, y)) = origin.filter(|_| part.starts_with('m')) {
            let mut tail = &part[1..];
            if let (Some(dx), Some(dy)) = (number(&mut tail), number(&mut tail)) {
                let implicit = tail
                    .trim_start()
                    .chars()
                    .next()
                    .is_some_and(|c| !c.is_ascii_alphabetic());
                kept.push_str(&format!(
                    "M{} {}{}{}",
                    x + dx,
                    y + dy,
                    if implicit { "l" } else { "" },
                    tail
                ));
                continue;
            }
        }
        kept.push_str(part);
    }
    (kept, dots)
}

pub(super) fn normalize(svg: &str) -> Result<String, String> {
    let mut reader = Reader::from_str(svg);
    let mut writer = Writer::new(Vec::with_capacity(svg.len()));
    let mut paints = vec![(1_f32, String::from("none"), false)];
    loop {
        let event = reader.read_event().map_err(|e| e.to_string())?;
        match &event {
            Event::Eof => break,
            Event::Start(tag) | Event::Empty(tag) => {
                let inherited = paints.last().expect("root paint");
                let width = attr(tag, "stroke-width")
                    .and_then(|v| v.parse::<f32>().ok())
                    .unwrap_or(inherited.0);
                let stroke = attr(tag, "stroke").unwrap_or_else(|| inherited.1.clone());
                let round = attr(tag, "stroke-linecap").map_or(inherited.2, |v| v == "round");
                if matches!(event, Event::Start(_)) {
                    paints.push((width, stroke, round));
                } else if round && width.is_finite() && width > 0. && stroke != "none" {
                    let (remaining, dots) = match tag.name().as_ref() {
                        b"path" => path_dots(&attr(tag, "d").unwrap_or_default()),
                        b"line" => {
                            let point = |key| {
                                attr(tag, key)
                                    .and_then(|v| v.parse::<f32>().ok())
                                    .unwrap_or(0.)
                            };
                            let from = (point("x1"), point("y1"));
                            let to = (point("x2"), point("y2"));
                            (
                                String::new(),
                                if tiny(from, to) { vec![from] } else { vec![] },
                            )
                        }
                        _ => (String::new(), vec![]),
                    };
                    if !dots.is_empty() {
                        if !remaining.trim().is_empty() {
                            let mut path = element(tag, "path", &["d"]);
                            path.push_attribute(("d", remaining.as_str()));
                            writer
                                .write_event(Event::Empty(path))
                                .map_err(|e| e.to_string())?;
                        }
                        for (x, y) in dots {
                            let mut circle = element(
                                tag,
                                "circle",
                                &["d", "x1", "x2", "y1", "y2", "fill", "stroke"],
                            );
                            let (x, y, radius) =
                                (x.to_string(), y.to_string(), (width / 2.).to_string());
                            circle.push_attribute(("cx", x.as_str()));
                            circle.push_attribute(("cy", y.as_str()));
                            circle.push_attribute(("r", radius.as_str()));
                            circle.push_attribute(("fill", stroke.as_str()));
                            circle.push_attribute(("stroke", "none"));
                            writer
                                .write_event(Event::Empty(circle))
                                .map_err(|e| e.to_string())?;
                        }
                        continue;
                    }
                }
            }
            Event::End(_) if paints.len() > 1 => {
                paints.pop();
            }
            _ => {}
        }
        writer.write_event(event).map_err(|e| e.to_string())?;
    }
    String::from_utf8(writer.into_inner()).map_err(|e| e.to_string())
}
