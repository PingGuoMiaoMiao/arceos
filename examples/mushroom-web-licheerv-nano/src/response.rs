use core::fmt::{self, Write};

use axmodel_mushroom_yolov5::{Detection, InferenceTiming};

use crate::page::INDEX_HTML;

pub trait ByteWriter {
    type Error;

    fn write_all(&mut self, bytes: &[u8]) -> Result<(), Self::Error>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HealthReport<'a> {
    pub wifi: &'a str,
    pub tpu: &'a str,
    pub model: &'a str,
}

#[derive(Clone, Copy, Debug)]
pub struct InferenceJson<'a> {
    pub request_id: u64,
    pub model: &'a str,
    pub source_width: u32,
    pub source_height: u32,
    pub detections: &'a [Detection],
    pub receive_us: u64,
    pub timing: InferenceTiming,
}

pub fn write_index_response<W: ByteWriter>(writer: &mut W) -> Result<(), W::Error> {
    write_head(writer, 200, "text/html; charset=utf-8", INDEX_HTML.len())?;
    writer.write_all(INDEX_HTML)
}

pub fn write_health_response<W: ByteWriter>(
    writer: &mut W,
    report: HealthReport<'_>,
) -> Result<(), W::Error> {
    write_json_response(writer, 200, |output| {
        write!(
            output,
            "{{\"wifi\":{},\"tpu\":{},\"model\":{}}}",
            JsonString(report.wifi),
            JsonString(report.tpu),
            JsonString(report.model)
        )
    })
}

pub fn write_error_response<W: ByteWriter>(
    writer: &mut W,
    status: u16,
    code: &str,
    message: &str,
) -> Result<(), W::Error> {
    write_json_response(writer, status, |output| {
        write!(
            output,
            "{{\"error\":{{\"code\":{},\"message\":{}}}}}",
            JsonString(code),
            JsonString(message)
        )
    })
}

pub fn write_inference_response<W: ByteWriter>(
    writer: &mut W,
    response: InferenceJson<'_>,
) -> Result<(), W::Error> {
    write_json_response(writer, 200, |output| {
        render_inference_json(output, response)
    })
}

fn render_inference_json<W: Write + ?Sized>(
    output: &mut W,
    response: InferenceJson<'_>,
) -> fmt::Result {
    write!(
        output,
        "{{\"request_id\":{},\"model\":{},\"image\":{{\"width\":{},\"height\":{}}},\"detections\":[",
        response.request_id,
        JsonString(response.model),
        response.source_width,
        response.source_height
    )?;
    for (index, detection) in response.detections.iter().enumerate() {
        if index != 0 {
            output.write_char(',')?;
        }
        write!(
            output,
            "{{\"label\":\"mushroom\",\"score\":{},\"x1\":{},\"y1\":{},\"x2\":{},\"y2\":{}}}",
            JsonFloat(detection.confidence),
            JsonFloat(detection.x1),
            JsonFloat(detection.y1),
            JsonFloat(detection.x2),
            JsonFloat(detection.y2)
        )?;
    }
    write!(
        output,
        "],\"timing_us\":{{\"receive\":{},\"quantize\":{},\"tpu\":{},\"postprocess\":{},\"total\":{}}}}}",
        response.receive_us,
        response.timing.quantize_us,
        response.timing.tpu_us,
        response.timing.postprocess_us,
        response.timing.total_us
    )
}

fn write_json_response<W, F>(writer: &mut W, status: u16, render: F) -> Result<(), W::Error>
where
    W: ByteWriter,
    F: Fn(&mut dyn Write) -> fmt::Result,
{
    let mut counter = CountingWriter::default();
    let _ = render(&mut counter);
    write_head(
        writer,
        status,
        "application/json; charset=utf-8",
        counter.length,
    )?;

    let mut adapter = FmtAdapter::new(writer);
    let _ = render(&mut adapter);
    adapter.finish()
}

fn write_head<W: ByteWriter>(
    writer: &mut W,
    status: u16,
    content_type: &str,
    content_length: usize,
) -> Result<(), W::Error> {
    let mut adapter = FmtAdapter::new(writer);
    let _ = write!(
        adapter,
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        status,
        reason_phrase(status),
        content_type,
        content_length
    );
    adapter.finish()
}

fn reason_phrase(status: u16) -> &'static str {
    match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        411 => "Length Required",
        413 => "Payload Too Large",
        415 => "Unsupported Media Type",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        _ => "Unknown",
    }
}

#[derive(Default)]
struct CountingWriter {
    length: usize,
}

impl Write for CountingWriter {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        self.length += value.len();
        Ok(())
    }
}

struct FmtAdapter<'a, W: ByteWriter> {
    writer: &'a mut W,
    error: Option<W::Error>,
}

impl<'a, W: ByteWriter> FmtAdapter<'a, W> {
    fn new(writer: &'a mut W) -> Self {
        Self {
            writer,
            error: None,
        }
    }

    fn finish(self) -> Result<(), W::Error> {
        match self.error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

impl<W: ByteWriter> Write for FmtAdapter<'_, W> {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        if self.error.is_some() {
            return Err(fmt::Error);
        }
        match self.writer.write_all(value.as_bytes()) {
            Ok(()) => Ok(()),
            Err(error) => {
                self.error = Some(error);
                Err(fmt::Error)
            }
        }
    }
}

struct JsonString<'a>(&'a str);

impl fmt::Display for JsonString<'_> {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        output.write_char('"')?;
        for character in self.0.chars() {
            match character {
                '"' => output.write_str("\\\""),
                '\\' => output.write_str("\\\\"),
                '\u{08}' => output.write_str("\\b"),
                '\u{0c}' => output.write_str("\\f"),
                '\n' => output.write_str("\\n"),
                '\r' => output.write_str("\\r"),
                '\t' => output.write_str("\\t"),
                value if value <= '\u{1f}' => write!(output, "\\u{:04x}", value as u32),
                value => output.write_char(value),
            }?;
        }
        output.write_char('"')
    }
}

struct JsonFloat(f32);

impl fmt::Display for JsonFloat {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_finite() {
            write!(output, "{:.6}", self.0)
        } else {
            output.write_str("null")
        }
    }
}
