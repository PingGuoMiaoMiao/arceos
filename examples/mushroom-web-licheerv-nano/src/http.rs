use crate::envelope::TOTAL_LENGTH;

pub const INFERENCE_CONTENT_TYPE: &str = "application/vnd.arceos.rgb-u8";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RequestRoute {
    Index,
    Health,
    Infer,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RequestHead {
    pub route: RequestRoute,
    pub content_length: usize,
    pub header_length: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HttpRejection {
    pub status: u16,
    pub code: &'static str,
}

const INVALID_HTTP: HttpRejection = HttpRejection {
    status: 400,
    code: "INVALID_HTTP",
};

pub fn parse_request_head(bytes: &[u8]) -> Result<RequestHead, HttpRejection> {
    let terminator = bytes
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or(INVALID_HTTP)?;
    let text = core::str::from_utf8(&bytes[..terminator]).map_err(|_| INVALID_HTTP)?;
    let mut lines = text.split("\r\n");
    let request_line = lines.next().ok_or(INVALID_HTTP)?;
    let mut request_parts = request_line.split(' ');
    let method = request_parts.next().ok_or(INVALID_HTTP)?;
    let path = request_parts.next().ok_or(INVALID_HTTP)?;
    let version = request_parts.next().ok_or(INVALID_HTTP)?;
    if request_parts.next().is_some() || version != "HTTP/1.1" {
        return Err(INVALID_HTTP);
    }

    let expected_method = match path {
        "/" => "GET",
        "/health" => "GET",
        "/api/infer" => "POST",
        _ => {
            return Err(HttpRejection {
                status: 404,
                code: "NOT_FOUND",
            });
        }
    };
    if method != expected_method {
        return Err(HttpRejection {
            status: 405,
            code: "METHOD_NOT_ALLOWED",
        });
    }

    let mut content_length = None;
    let mut content_type = None;
    for line in lines {
        let (name, value) = line.split_once(':').ok_or(INVALID_HTTP)?;
        let value = value.trim();
        if name.eq_ignore_ascii_case("Content-Length") {
            if content_length.is_some() {
                return Err(INVALID_HTTP);
            }
            content_length = Some(value.parse::<usize>().map_err(|_| INVALID_HTTP)?);
        } else if name.eq_ignore_ascii_case("Content-Type") {
            if content_type.is_some() {
                return Err(INVALID_HTTP);
            }
            content_type = Some(value);
        }
    }

    let route = match path {
        "/" => RequestRoute::Index,
        "/health" => RequestRoute::Health,
        "/api/infer" => RequestRoute::Infer,
        _ => unreachable!(),
    };
    if route != RequestRoute::Infer {
        return Ok(RequestHead {
            route,
            content_length: content_length.unwrap_or(0),
            header_length: terminator + 4,
        });
    }

    let content_length = content_length.ok_or(HttpRejection {
        status: 411,
        code: "LENGTH_REQUIRED",
    })?;
    if content_length > TOTAL_LENGTH {
        return Err(HttpRejection {
            status: 413,
            code: "PAYLOAD_TOO_LARGE",
        });
    }
    if content_length != TOTAL_LENGTH {
        return Err(HttpRejection {
            status: 400,
            code: "INVALID_LENGTH",
        });
    }
    if content_type != Some(INFERENCE_CONTENT_TYPE) {
        return Err(HttpRejection {
            status: 415,
            code: "UNSUPPORTED_MEDIA_TYPE",
        });
    }

    Ok(RequestHead {
        route,
        content_length,
        header_length: terminator + 4,
    })
}
