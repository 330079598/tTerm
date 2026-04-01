use tokio::io::AsyncWriteExt;

/// Process SSH output for terminal queries and write replies back through SSH channel
pub async fn process_ssh_output_for_ui<W>(
    data: &[u8],
    writer: &mut W,
) -> std::io::Result<String>
where
    W: AsyncWriteExt + Unpin,
{
    let mut output = Vec::new();
    let mut i = 0;
    
    while i < data.len() {
        if data[i] != 0x1b {
            output.push(data[i]);
            i += 1;
            continue;
        }
        
        let rest = &data[i..];
        let mut matched = false;
        
        // Secondary DA: ESC[>c
        if rest.len() >= 4 && rest.starts_with(b"\x1b[>") {
            if let Some(end) = rest.iter().position(|&b| b == b'c') {
                writer.write_all(b"\x1b[>0;276;0c").await?;
                writer.flush().await?;
                i += end + 1;
                matched = true;
            }
        }
        
        // CPR: ESC[6n
        if !matched && rest.starts_with(b"\x1b[6n") {
            writer.write_all(b"\x1b[1;1R").await?;
            writer.flush().await?;
            i += 4;
            matched = true;
        }
        
        // DEC CPR: ESC[?6n
        if !matched && rest.starts_with(b"\x1b[?6n") {
            writer.write_all(b"\x1b[?1;1R").await?;
            writer.flush().await?;
            i += 5;
            matched = true;
        }
        
        // OSR: ESC[5n
        if !matched && rest.starts_with(b"\x1b[5n") {
            writer.write_all(b"\x1b[0n").await?;
            writer.flush().await?;
            i += 4;
            matched = true;
        }
        
        // Primary DA: ESC[c or ESC[?c
        if !matched && rest.len() >= 3 && rest.starts_with(b"\x1b[") {
            let start = if rest[2] == b'?' { 3 } else { 2 };
            if rest.len() > start && rest[start] == b'c' {
                writer.write_all(b"\x1b[?64;1;2;4;6;9;15;21;22c").await?;
                writer.flush().await?;
                i += start + 1;
                matched = true;
            }
        }
        
        // DCS XTGETTCAP: ESC P + q <hex> ESC \
        if !matched && rest.starts_with(b"\x1bP+q") {
            if let Some(end_pos) = find_dcs_end(rest) {
                let hex_query = &rest[4..end_pos];
                
                // Check if it's asking for "Co" (colors)
                if hex_query == b"436f" || hex_query == b"436F" {
                    writer.write_all(b"\x1bP1+r436f3d323536\x1b\\").await?;
                } else {
                    // Unknown capability
                    writer.write_all(b"\x1bP0+r").await?;
                    writer.write_all(hex_query).await?;
                    writer.write_all(b"\x1b\\").await?;
                }
                writer.flush().await?;
                i += end_pos + 2;
                matched = true;
            }
        }
        
        // OSC color query: ESC]10;? or ESC]11;?
        if !matched && rest.starts_with(b"\x1b]") {
            if let Some(end_pos) = find_osc_end(rest) {
                let body = &rest[2..end_pos];
                if body.starts_with(b"10;?") {
                    writer.write_all(b"\x1b]10;rgb:aaaa/aaaa/aaaa\x1b\\").await?;
                    writer.flush().await?;
                    i += end_pos + 2;
                    matched = true;
                } else if body.starts_with(b"11;?") {
                    writer.write_all(b"\x1b]11;rgb:1111/1111/1111\x1b\\").await?;
                    writer.flush().await?;
                    i += end_pos + 2;
                    matched = true;
                }
            }
        }
        
        if !matched {
            output.push(data[i]);
            i += 1;
        }
    }
    
    Ok(String::from_utf8_lossy(&output).into_owned())
}

fn find_dcs_end(data: &[u8]) -> Option<usize> {
    for i in 2..data.len() {
        if data[i] == 0x9c {
            return Some(i);
        }
        if data[i] == 0x1b && i + 1 < data.len() && data[i + 1] == b'\\' {
            return Some(i);
        }
    }
    None
}

fn find_osc_end(data: &[u8]) -> Option<usize> {
    for i in 2..data.len() {
        if data[i] == 0x07 {
            return Some(i);
        }
        if data[i] == 0x1b && i + 1 < data.len() && data[i + 1] == b'\\' {
            return Some(i);
        }
    }
    None
}
