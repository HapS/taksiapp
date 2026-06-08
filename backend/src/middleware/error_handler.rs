use axum::{
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    Json,
};
use serde_json::json;
use tera::Error as TeraError;

// Escape HTML special characters so raw Tera error text can be safely rendered
// inside the debug error page (<pre><code>).
fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Handle template errors with detailed information in debug mode
///
/// When debug=true in config.toml: Shows detailed Tera error page (raw details)
/// When debug=false in config.toml: Shows generic error page for security
pub fn handle_template_error(err: &TeraError, is_debug: bool) -> Response {
    // Backwards-compatible wrapper: no snippet, no JSON preference, no AppState context
    handle_template_error_with_context(err, is_debug, false, None)
}

pub fn handle_template_error_with_context(
    err: &TeraError,
    is_debug: bool,
    prefer_json: bool,
    state: Option<&crate::app_state::AppState>,
) -> Response {
    // Try to extract template name and line number from Tera's message (best-effort)
    let template_name = extract_template_name(&err.to_string());
    let line_no = extract_line_number(&err.to_string());

    // Generate snippet only in debug mode and only if we have a template name
    let snippet_html = if is_debug {
        if let Some(ref name) = template_name {
            generate_snippet_html(state, name, line_no, 3).unwrap_or_default()
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    // If API caller expects JSON (or controller indicated JSON preference), return JSON
    if prefer_json {
        let payload = if is_debug {
            json!({
                "error": "template_render_error",
                "message": err.to_string(),
                "details": format!("{:#?}", err),
                "template": template_name,
                "line": line_no,
                "snippet": if snippet_html.is_empty() { serde_json::Value::Null } else { json!(snippet_html) }
            })
        } else {
            json!({
                "error": "template_render_error",
                "message": "Internal Server Error"
            })
        };
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(payload)).into_response();
    }

    // HTML response — include snippet if any
    let snippet_block = if snippet_html.is_empty() {
        "".to_string()
    } else {
        format!(
            r#"
        <div class="details">
            <h3>🔎 Template Snippet</h3>
            <pre style="white-space: pre-wrap; word-wrap: break-word;"><code>{}</code></pre>
        </div>
        "#,
            snippet_html
        )
    };

    // Render the detailed HTML page (debug) or generic page (prod)
    if is_debug {
        let html = format!(
            r#"
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Template Error - Backend RS</title>
    <style>
        * {{
            margin: 0;
            padding: 0;
            box-sizing: border-box;
        }}
        body {{
            font-family: 'Segoe UI', Tahoma, Geneva, Verdana, sans-serif;
            background: #1e1e1e;
            color: #d4d4d4;
            padding: 20px;
            line-height: 1.6;
        }}
        .container {{
            max-width: 1200px;
            margin: 0 auto;
        }}
        h1 {{
            color: #f44336;
            font-size: 2.5em;
            margin-bottom: 20px;
            display: flex;
            align-items: center;
            gap: 15px;
        }}
        .error-icon {{
            font-size: 1.2em;
        }}
        .error-box {{
            background: linear-gradient(135deg, #f44336 0%, #d32f2f 100%);
            color: white;
            padding: 25px;
            border-radius: 8px;
            margin-bottom: 20px;
            box-shadow: 0 4px 6px rgba(0, 0, 0, 0.3);
        }}
        .error-box h2 {{
            font-size: 1.5em;
            margin-bottom: 10px;
        }}
        .error-type {{
            display: inline-block;
            background: rgba(255, 255, 255, 0.2);
            padding: 5px 15px;
            border-radius: 20px;
            font-size: 0.9em;
            margin-bottom: 15px;
        }}
        .details {{
            background: #2d2d2d;
            padding: 20px;
            margin-top: 20px;
            border-left: 4px solid #f44336;
            border-radius: 4px;
        }}
        .details h3 {{
            color: #4fc3f7;
            margin-bottom: 15px;
            font-size: 1.2em;
        }}
        .info-grid {{
            display: grid;
            grid-template-columns: 150px 1fr;
            gap: 10px;
            margin-bottom: 15px;
        }}
        .info-label {{
            color: #9e9e9e;
            font-weight: 600;
        }}
        .info-value {{
            color: #fff;
            font-family: 'Courier New', monospace;
            background: #1e1e1e;
            padding: 5px 10px;
            border-radius: 3px;
        }}
        pre {{
            background: #1e1e1e;
            padding: 15px;
            overflow-x: auto;
            border-radius: 4px;
            border: 1px solid #424242;
            margin-top: 10px;
        }}
        code {{
            color: #4fc3f7;
            font-family: 'Courier New', monospace;
        }}
        .suggestions {{
            background: #2d2d2d;
            padding: 20px;
            margin-top: 20px;
            border-left: 4px solid #4caf50;
            border-radius: 4px;
        }}
        .suggestions h3 {{
            color: #4caf50;
            margin-bottom: 15px;
        }}
        .suggestions ul {{
            list-style: none;
            padding-left: 0;
        }}
        .suggestions li {{
            padding: 8px 0;
            padding-left: 25px;
            position: relative;
        }}
        .suggestions li:before {{
            content: "→";
            position: absolute;
            left: 0;
            color: #4caf50;
        }}
        .footer {{
            margin-top: 30px;
            padding-top: 20px;
            border-top: 1px solid #424242;
            color: #9e9e9e;
            font-size: 0.9em;
        }}
    </style>
</head>
<body>
    <div class="container">
        <h1>
            <span class="error-icon">🔥</span>
            Template Rendering Error
        </h1>

        <div class="error-box">
            <div class="error-type">{}</div>
            <h2>{}</h2>
        </div>

        <div class="details">
            <h3>📋 Error Details</h3>
            <pre style="white-space: pre-wrap; word-wrap: break-word;"><code>{}</code></pre>
        </div>

        {}

        {}

        <div class="footer">
            <p>💡 This detailed error page is shown because <code>debug = true</code> in config.toml</p>
            <p>Set <code>debug = false</code> in production to show generic error pages.</p>
            <p style="margin-top: 10px; color: #f44336;">
                <strong>⚠️  Security Warning:</strong> Never enable debug mode in production!
            </p>
        </div>
    </div>
</body>
</html>
"#,
            get_error_type(err),
            escape_html(&err.to_string()),
            get_error_details(err),
            snippet_block,
            get_suggestions(err)
        );

        (StatusCode::INTERNAL_SERVER_ERROR, Html(html)).into_response()
    } else {
        // Production: Generic error page (debug = false)
        let html = r#"
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Error - Backend RS</title>
    <style>
        * {
            margin: 0;
            padding: 0;
            box-sizing: border-box;
        }
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, Cantarell, sans-serif;
            display: flex;
            justify-content: center;
            align-items: center;
            min-height: 100vh;
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            padding: 20px;
        }
        .error-container {
            text-align: center;
            padding: 60px 40px;
            background: white;
            border-radius: 20px;
            box-shadow: 0 20px 60px rgba(0,0,0,0.3);
            max-width: 500px;
            width: 100%;
        }
        .error-icon {
            font-size: 5em;
            margin-bottom: 20px;
        }
        h1 {
            color: #f44336;
            font-size: 4em;
            margin-bottom: 10px;
            font-weight: 700;
        }
        .error-title {
            color: #333;
            font-size: 1.5em;
            margin-bottom: 15px;
            font-weight: 600;
        }
        p {
            color: #666;
            font-size: 1.1em;
            line-height: 1.6;
            margin-bottom: 30px;
        }
        .btn {
            display: inline-block;
            padding: 12px 30px;
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            color: white;
            text-decoration: none;
            border-radius: 25px;
            font-weight: 600;
            transition: transform 0.2s, box-shadow 0.2s;
        }
        .btn:hover {
            transform: translateY(-2px);
            box-shadow: 0 5px 20px rgba(102, 126, 234, 0.4);
        }
        .footer {
            margin-top: 30px;
            padding-top: 20px;
            border-top: 1px solid #eee;
            color: #999;
            font-size: 0.9em;
        }
    </style>
</head>
<body>
    <div class="error-container">
        <div class="error-icon">😔</div>
        <h1>500</h1>
        <div class="error-title">Internal Server Error</div>
        <p>Oops! Something went wrong on our end. We're working to fix it.</p>
        <a href="/" class="btn">← Go Back Home</a>
        <div class="footer">
            If the problem persists, please contact support.
        </div>
    </div>
</body>
</html>
"#;
        (StatusCode::INTERNAL_SERVER_ERROR, Html(html)).into_response()
    }
}

fn get_error_type(err: &TeraError) -> &str {
    let msg = err.to_string().to_lowercase();
    if msg.contains("not found") || msg.contains("doesn't exist") {
        "Template Not Found"
    } else if msg.contains("syntax") || msg.contains("unexpected") {
        "Syntax Error"
    } else if msg.contains("variable") || msg.contains("not defined") || msg.contains("undefined") {
        "Missing Variable"
    } else {
        "Render Error"
    }
}

fn get_error_details(err: &TeraError) -> String {
    // Show the Display representation and a Debug dump for full fidelity.
    // Escape HTML so raw Tera output is safely displayed inside <pre><code>.
    let display = escape_html(&format!("{}", err));
    let debug = escape_html(&format!("{:#?}", err));
    format!("{}\n\n--- Debug Information ---\n{}", display, debug)
}

fn extract_template_name(err_msg: &str) -> Option<String> {
    // Heuristic parse: look for a quoted template name after the word `template`
    if let Some(start) = err_msg.find("template") {
        let tail = &err_msg[start..];
        if let Some(qpos) = tail.find('\'').or_else(|| tail.find('"')) {
            let name_start = start + qpos + 1;
            if let Some(name_end_rel) = err_msg[name_start..]
                .find('\'')
                .or_else(|| err_msg[name_start..].find('"'))
            {
                return Some(err_msg[name_start..name_start + name_end_rel].to_string());
            }
        }
    }
    None
}

fn extract_line_number(err_msg: &str) -> Option<usize> {
    if let Some(start) = err_msg.find("line") {
        let after = &err_msg[start + 4..];
        if let Some(num_str) = after.split_whitespace().next() {
            let digits = num_str.trim_matches(|c: char| !c.is_numeric());
            if let Ok(n) = digits.parse::<usize>() {
                return Some(n);
            }
        }
    }
    None
}

fn generate_snippet_html(
    state: Option<&crate::app_state::AppState>,
    template_name: &str,
    line: Option<usize>,
    radius: usize,
) -> Option<String> {
    // Try to read the source either via AppState helper or directly from disk
    let source_opt = if let Some(st) = state {
        st.read_template_source(template_name)
    } else {
        let path = std::path::Path::new("templates").join(template_name);
        if path.is_file() {
            std::fs::read_to_string(path).ok()
        } else {
            None
        }
    };

    let source = source_opt?;
    let lines: Vec<&str> = source.lines().collect();
    if lines.is_empty() {
        return None;
    }
    let total = lines.len();
    let line_idx = line.unwrap_or(1).saturating_sub(1);
    let start = if line_idx > radius {
        line_idx - radius
    } else {
        0
    };
    let end = std::cmp::min(total.saturating_sub(1), line_idx + radius);

    let mut out = String::new();
    for i in start..=end {
        let l = lines.get(i).unwrap_or(&"");
        let esc = escape_html(l);
        if i == line_idx {
            out.push_str(&format!(
                r#"<div style="background:#2b2b2b;padding:6px;border-left:4px solid #f44336;"><span style="color:#888;margin-right:8px">{}</span><code>{}</code></div>"#,
                i + 1,
                esc
            ));
        } else {
            out.push_str(&format!(
                r#"<div><span style="color:#888;margin-right:8px">{}</span><code>{}</code></div>"#,
                i + 1,
                esc
            ));
        }
    }
    Some(out)
}

fn get_suggestions(err: &TeraError) -> String {
    let msg = err.to_string().to_lowercase();
    let mut suggestions: Vec<String> = Vec::new();

    if msg.contains("not found") || msg.contains("doesn't exist") {
        suggestions
            .push("Check if the template file exists in the `templates/` directory".to_string());
        suggestions.push("Verify includes and extends paths are correct".to_string());
        suggestions.push("Check filename casing (on case-sensitive filesystems)".to_string());
    } else if msg.contains("variable") || msg.contains("undefined") || msg.contains("not defined") {
        suggestions
            .push("Ensure the missing variable is inserted into the template context".to_string());
        suggestions
            .push("Check for typos in variable names in both template and controller".to_string());
        suggestions.push("Wrap accesses in `if` checks if they may be optional".to_string());
    } else if msg.contains("syntax") || msg.contains("unexpected") {
        suggestions.push("Check for unclosed tags or missing braces".to_string());
        suggestions.push("Validate included and extended templates for syntax errors".to_string());
    } else {
        suggestions.push("Inspect the server logs for the full error trace".to_string());
        suggestions.push("Try restarting the application to clear caches".to_string());
    }

    format!(
        r#"
        <div class="suggestions">
            <h3>💡 Suggestions</h3>
            <ul>
                {}
            </ul>
        </div>
        "#,
        suggestions
            .iter()
            .map(|s| format!("<li>{}</li>", s))
            .collect::<Vec<_>>()
            .join("\n")
    )
}
