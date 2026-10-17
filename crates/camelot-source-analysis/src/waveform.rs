use crate::dsp::{waveform_context_rust_v1 as context, waveform_front_end_rust_v1 as frontend};
use anyhow::{Result, ensure};
use std::io::{self, Write};

struct ContextSink {
    context: context::Context,
    rows: Vec<context::Features>,
}

impl Write for ContextSink {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() != frontend::ROW_VALUES * 4 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "waveform row shape",
            ));
        }
        let row: Vec<f32> = bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|v| f32::from_le_bytes(*v))
            .collect();
        if let Some(features) = self.context.feed(&row).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("waveform context: {error:?}"),
            )
        })? {
            self.rows.push(features);
        }
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub(crate) fn analyze(pcm: &[f32]) -> Result<Vec<Vec<f32>>> {
    let mut sink = ContextSink {
        context: context::Context::new(),
        rows: Vec::new(),
    };
    let geometry = frontend::analyze_pcm(pcm, &mut sink).map_err(anyhow::Error::msg)?;
    sink.rows.extend(
        sink.context
            .finish()
            .map_err(|e| anyhow::anyhow!("waveform finish: {e:?}"))?,
    );
    ensure!(
        sink.rows.len() == geometry.row_count,
        "waveform context row count differs"
    );
    let mut arrays = (0..8)
        .map(|_| Vec::with_capacity(sink.rows.len()))
        .collect::<Vec<_>>();
    for row in sink.rows {
        for (output, value) in arrays.iter_mut().zip(row) {
            output.push(value);
        }
    }
    Ok(arrays)
}
