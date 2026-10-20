//! Shot strings from chronograph files: Garmin Xero (the ShotView app's
//! CSV), LabRadar (the SR…Report.csv on its card) and MagnetoSpeed (its
//! LOG.CSV, several series in one). The format is told from the content.
//! The files' own averages are rounded, localised and sometimes "nan", so
//! the statistics are worked out here from the shots.

use crate::model::ShotStats;

const MPS_PER_FPS: f64 = 0.3048;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Device {
    Garmin,
    LabRadar,
    MagnetoSpeed,
}

impl Device {
    pub fn name(self) -> &'static str {
        match self {
            Device::Garmin => "Garmin Xero",
            Device::LabRadar => "LabRadar",
            Device::MagnetoSpeed => "MagnetoSpeed",
        }
    }
}

/// One string of shots.
#[derive(Clone, Debug, PartialEq)]
pub struct Session {
    pub device: Device,
    /// The session's or series' name, if the file has one.
    pub name: Option<String>,
    /// Muzzle velocities, m/s, misreads left out.
    pub velocities_mps: Vec<f64>,
    /// Shots the chronograph didn't read (zero or empty).
    pub missed: usize,
}

impl Session {
    pub fn average_mps(&self) -> f64 {
        self.velocities_mps.iter().sum::<f64>() / self.velocities_mps.len().max(1) as f64
    }

    pub fn stats(&self) -> ShotStats {
        let count = self.velocities_mps.len();
        let average = self.average_mps();
        let sd = if count > 1 {
            (self
                .velocities_mps
                .iter()
                .map(|v| (v - average).powi(2))
                .sum::<f64>()
                / (count - 1) as f64)
                .sqrt()
        } else {
            0.0
        };
        let (low, high) = self
            .velocities_mps
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(low, high), v| {
                (low.min(*v), high.max(*v))
            });
        ShotStats {
            count: count as u32,
            sd_mps: sd,
            es_mps: if count > 0 { high - low } else { 0.0 },
        }
    }
}

/// Reads a chronograph file. Several sessions for a file with several
/// series; an error says what the file is not.
pub fn parse(bytes: &[u8]) -> Result<Vec<Session>, String> {
    let text = String::from_utf8_lossy(bytes)
        .replace(['\u{feff}', '\0'], "")
        .replace("\r\n", "\n")
        .replace('\r', "\n");
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let sessions = if lines.iter().any(|line| is_garmin_header(line)) {
        garmin(&lines)?
    } else if lines.iter().any(|line| line.starts_with("Shot ID")) {
        labradar(&lines)?
    } else if lines
        .iter()
        .any(|line| line.starts_with("Series,") && line.contains("Shots:"))
    {
        magnetospeed(&lines)?
    } else {
        return Err(
            "not a chronograph file this app knows (Garmin Xero ShotView CSV, \
                    LabRadar SR…Report.csv or MagnetoSpeed LOG.CSV)"
                .into(),
        );
    };
    let sessions: Vec<Session> = sessions
        .into_iter()
        .filter(|session| !session.velocities_mps.is_empty())
        .collect();
    if sessions.is_empty() {
        return Err("the file has no shots with a velocity".into());
    }
    Ok(sessions)
}

/// Splits a CSV line on `separator`, honouring quotes, trimming cells.
fn cells(line: &str, separator: char) -> Vec<String> {
    let mut cells = Vec::new();
    let mut cell = String::new();
    let mut quoted = false;
    for c in line.chars() {
        match c {
            '"' => quoted = !quoted,
            c if c == separator && !quoted => cells.push(std::mem::take(&mut cell)),
            c => cell.push(c),
        }
    }
    cells.push(cell);
    cells
        .into_iter()
        .map(|cell| cell.trim().to_owned())
        .collect()
}

/// A number with a decimal point or comma.
fn number(cell: &str) -> Option<f64> {
    cell.trim()
        .replace(',', ".")
        .parse()
        .ok()
        .filter(|v: &f64| v.is_finite())
}

/// m/s per unit of a speed unit as files write it.
fn speed_unit(unit: &str) -> Option<f64> {
    match unit.trim().to_lowercase().replace(['(', ')'], "").as_str() {
        "fps" | "ft/s" => Some(MPS_PER_FPS),
        "mps" | "m/s" => Some(1.0),
        "km/h" | "kmh" => Some(1.0 / 3.6),
        "mph" => Some(0.447_04),
        "yps" | "yd/s" => Some(0.9144),
        _ => None,
    }
}

/// "#,SPEED (FPS),…" in any language: the shot table's header.
fn is_garmin_header(line: &str) -> bool {
    line.starts_with("#,")
        && cells(line, ',')
            .get(1)
            .is_some_and(|cell| garmin_unit(cell).is_some())
}

fn garmin_unit(cell: &str) -> Option<f64> {
    let upper = cell.to_uppercase();
    let start = upper.rfind('(')?;
    let end = upper[start..].find(')')? + start;
    speed_unit(&upper[start + 1..end])
}

fn garmin(lines: &[&str]) -> Result<Vec<Session>, String> {
    let header = lines
        .iter()
        .position(|line| is_garmin_header(line))
        .ok_or("no shot table")?;
    let unit = garmin_unit(&cells(lines[header], ',')[1]).ok_or("unknown speed unit")?;
    // The first line is the session's title, written raw (commas and all).
    let name = (header > 0)
        .then(|| lines[0].trim_matches('"').trim().to_owned())
        .filter(|name| !name.is_empty());
    let mut session = Session {
        device: Device::Garmin,
        name,
        velocities_mps: Vec::new(),
        missed: 0,
    };
    for line in &lines[header + 1..] {
        if line.starts_with('-') {
            break;
        }
        let row = cells(line, ',');
        if row.first().is_none_or(|cell| cell.parse::<u32>().is_err()) {
            continue;
        }
        match row.get(1).and_then(|cell| number(cell)) {
            Some(speed) if speed > 0.0 => session.velocities_mps.push(speed * unit),
            _ => session.missed += 1,
        }
    }
    Ok(vec![session])
}

fn labradar(lines: &[&str]) -> Result<Vec<Session>, String> {
    let separator = if lines
        .first()
        .is_some_and(|line| line.starts_with("sep=") && !line.contains(';'))
    {
        ','
    } else {
        ';'
    };
    let field = |name: &str| {
        lines
            .iter()
            .map(|line| cells(line, separator))
            .find(|row| row.first().is_some_and(|cell| cell == name))
            .and_then(|row| row.get(1).cloned())
    };
    let unit = match field("Units velocity") {
        Some(unit) => speed_unit(&unit).ok_or(format!("unknown speed unit: {unit}"))?,
        None => MPS_PER_FPS,
    };
    let header_line = lines
        .iter()
        .position(|line| line.starts_with("Shot ID"))
        .ok_or("no shot table")?;
    let header = cells(lines[header_line], separator);
    let v0 = header
        .iter()
        .position(|cell| cell == "V0")
        .ok_or("no V0 (muzzle velocity) column")?;
    let mut session = Session {
        device: Device::LabRadar,
        name: field("Series No").map(|series| format!("Series {series}")),
        velocities_mps: Vec::new(),
        missed: 0,
    };
    for line in &lines[header_line + 1..] {
        let row = cells(line, separator);
        if row.first().is_none_or(|cell| cell.is_empty()) {
            continue;
        }
        match row.get(v0).and_then(|cell| number(cell)) {
            Some(speed) if speed > 0.0 => session.velocities_mps.push(speed * unit),
            _ => session.missed += 1,
        }
    }
    Ok(vec![session])
}

fn magnetospeed(lines: &[&str]) -> Result<Vec<Session>, String> {
    let mut sessions = Vec::new();
    let mut current: Option<Session> = None;
    for line in lines {
        let row = cells(line, ',');
        let first = row.first().map(String::as_str).unwrap_or("");
        if first == "Series" && row.get(2).is_some_and(|cell| cell.starts_with("Shots")) {
            sessions.extend(current.take());
            current = Some(Session {
                device: Device::MagnetoSpeed,
                name: row.get(1).map(|series| format!("Series {series}")),
                velocities_mps: Vec::new(),
                missed: 0,
            });
            continue;
        }
        if line.starts_with("----") {
            sessions.extend(current.take());
            continue;
        }
        // Shot rows: series, shot, speed, unit.
        let Some(session) = current.as_mut() else {
            continue;
        };
        if row.len() >= 4 && row[0].parse::<u32>().is_ok() && row[1].parse::<u32>().is_ok() {
            let unit = speed_unit(&row[3]).unwrap_or(MPS_PER_FPS);
            match number(&row[2]) {
                Some(speed) if speed > 0.0 => session.velocities_mps.push(speed * unit),
                _ => session.missed += 1,
            }
        }
    }
    sessions.extend(current);
    Ok(sessions)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Excerpts of real files from MIT-licensed projects, kept as test data:
    // useyournose/xeroconv (Copyright (c) 2024 useyournose) and
    // ammolytics/experiments (Copyright (c) 2018 ammolytics). Permission is
    // hereby granted, free of charge, to any person obtaining a copy of this
    // software and associated documentation files, to deal in the Software
    // without restriction, subject to the copyright and permission notices.

    const GARMIN_IOS: &str = "\u{feff}\"105 hybrid 31.0gt\"\n\
#,SPEED (FPS),\u{394} AVG (FPS),KE (FT-LB),POWER FACTOR (KGR\u{22c5}FT/S),TIME,CLEAN BORE,COLD BORE,SHOT NOTES\n\
1, 2808.2, 13.9, , , 1:41:26 PM, , , \"\" \n\
2, 2790.7, -3.6, , , 1:41:52 PM, , , \"\" \n\
3, 2806.2, 12.0, , , 1:42:11 PM, , , \"\" \n\
-,,,,,,\n\
AVERAGE SPEED,2794.3,,,,,,,\n\
STD DEV,8.5,,,,,,,\n\
SPREAD,31.7,,,,,,,\n\
SESSION NOTE,\"\",,,,,,,\n\
-,,,,,,\n\
DATE, \"April 6, 2024 at 1:36 PM\",,,,,\n\
All shots included in the calculations,,,,,,,\u{feff}";

    const GARMIN_ANDROID_DE: &str = "Marlin,.357, 15,3grs W296, 158,0 gr\n\
\u{feff}#,Tempo (MPS),\u{394} DURCHSCHNITT (MPS),KE (J),Leistungsfaktor (N\u{22c5}s),Zeit,\u{d6}lschuss,Erstschuss,Schussnotizen\n\
1,\"488,3\",\"14,2\",\"1220,7\",\"5,0\",17:09:22,,,\n\
2,\"425,5\",\"-48,7\",\"926,7\",\"4,4\",17:09:27,,,\n\
3,\"477,0\",\"2,9\",\"1165,0\",\"4,9\",17:09:30,,,\n\
4,\"485,1\",\"11,0\",\"1204,9\",\"5,0\",17:09:34,,,\n\
5,\"476,8\",\"2,7\",\"1164,0\",\"4,9\",17:09:37,,,\n\
6,\"474,4\",\"0,3\",\"1152,1\",\"4,9\",17:10:04,,,\n\
7,\"470,7\",\"-3,5\",\"1134,0\",\"4,8\",17:10:08,,,\n\
8,\"474,8\",\"0,7\",\"1154,1\",\"4,9\",17:10:10,,,\n\
9,\"482,7\",\"8,5\",\"1192,7\",\"4,9\",17:10:13,,,\n\
10,\"486,0\",\"11,9\",\"1209,3\",\"5,0\",17:10:17,,,\n\
-,,,,,,\n\
AVERAGE SPEED,\"474,1\",,,,,\n\
STANDARDABWEICHUNG,\"17,1\",,,,,\n\
STREUUNG,\"62,8\",,,,,\n\
-,,,,,,\n\
Datum,\"JULI 02, 2024 17:08\",,,,,\n";

    const LABRADAR: &str = "sep=;\r\n\
Device ID;LBR-0028391;;\0          \r\n\
\0\r\n\
\0Series No;0003;;\0      \r\n\
\0Total number of shots;0023;;\0\r\n\
\0\r\n\
\0Units velocity;fps;;\0\r\n\
\0Units distances;yd;;\0\r\n\
\0Stats - Average;nan;fps;\0\r\n\
\0\r\n\
Shot ID;V0;V10;V20;V30;V40;V50;Ke0;Ke10;Ke20;Ke30;Ke40;Ke50;PF10;Proj. Weight;Date;Time\r\n\
0009;2047;0;2047;0;0;0;1673;0;1673;0;0;0;0.00;180.00;01-19-2019;11:25:35;\r\n\
0010;0;0;0;0;0;0;0;0;0;0;0;0;0.00;180.00;01-19-2019;11:26:12;\r\n\
0011;2770;2756;2742;2726;2712;2701;3066;3034;3004;2970;2938;2914;496.08;180.00;01-19-2019;11:26:51;\r\n";

    const MAGNETOSPEED: &str = "Series, 1, Shots:,3\r\n\
Min,2725, Max,2808  \r\n\
Avg,2767 ,S-D,41.8  \r\n\
ES ,  83\r\n\
\r\n\
Series,Shot,Speed   \r\n\
 1, 1, 2725, ft/s   \r\n\
 1, 2, 2769, ft/s   \r\n\
 1, 3, 2808, ft/s   \r\n\
----,----,----,---- \r\n\
Series, 2, Shots:,2\r\n\
\r\n\
Series,Shot,Speed   \r\n\
 2, 1, 2775, ft/s   \r\n\
 2, 2, 2777, ft/s   \r\n\
----,----,----,---- \r\n";

    #[test]
    fn garmin_ios_in_feet_per_second() {
        let sessions = parse(GARMIN_IOS.as_bytes()).unwrap();
        let session = &sessions[0];
        assert_eq!(session.device, Device::Garmin);
        assert_eq!(session.name.as_deref(), Some("105 hybrid 31.0gt"));
        assert_eq!(session.velocities_mps.len(), 3);
        assert!((session.velocities_mps[0] - 2808.2 * 0.3048).abs() < 1e-9);
    }

    #[test]
    fn garmin_android_with_decimal_commas() {
        let sessions = parse(GARMIN_ANDROID_DE.as_bytes()).unwrap();
        let session = &sessions[0];
        assert_eq!(
            session.name.as_deref(),
            Some("Marlin,.357, 15,3grs W296, 158,0 gr")
        );
        assert_eq!(session.velocities_mps.len(), 10);
        assert!((session.average_mps() - 474.13).abs() < 0.01);
        let stats = session.stats();
        assert!((stats.es_mps - 62.8).abs() < 1e-9);
        // The app writes the population SD (17.1); a sample's is larger.
        assert!((stats.sd_mps - 18.04).abs() < 0.01, "{}", stats.sd_mps);
    }

    #[test]
    fn labradar_leaves_out_misreads() {
        let sessions = parse(LABRADAR.as_bytes()).unwrap();
        let session = &sessions[0];
        assert_eq!(session.device, Device::LabRadar);
        assert_eq!(session.name.as_deref(), Some("Series 0003"));
        assert_eq!(session.velocities_mps.len(), 2);
        assert_eq!(session.missed, 1);
        assert!((session.velocities_mps[1] - 2770.0 * 0.3048).abs() < 1e-9);
    }

    #[test]
    fn magnetospeed_series_are_sessions() {
        let sessions = parse(MAGNETOSPEED.as_bytes()).unwrap();
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0].velocities_mps.len(), 3);
        assert_eq!(sessions[1].name.as_deref(), Some("Series 2"));
        assert!((sessions[1].average_mps() - 2776.0 * 0.3048).abs() < 1e-9);
    }

    #[test]
    fn other_files_are_refused() {
        assert!(parse(b"name,velocity\nfoo,800\n").is_err());
        assert!(parse(b"").is_err());
        assert!(parse("#,SPEED (MPS)\n-,,,\n".as_bytes()).is_err());
    }
}
