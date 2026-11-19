use crate::message::{Message, MessageDirection, MessageType};
use std::path::Path;

/// 导出格式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Txt,
    Json,
    Csv,
}

impl ExportFormat {
    /// 根据文件扩展名推断导出格式
    pub fn from_extension(path: &Path) -> Option<Self> {
        match path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .as_deref()
        {
            Some("txt") => Some(Self::Txt),
            Some("json") => Some(Self::Json),
            Some("csv") => Some(Self::Csv),
            _ => None,
        }
    }
}

/// 将消息列表导出为纯文本格式
///
/// `view` 是导出内容使用的视图（文本 / 十六进制），取自该连接当前
/// 「显示」选择，因此导出内容与界面上看到的一致。
///
/// 格式示例：
/// ```text
/// [发送] 2024-01-01 12:00:00.123 Hello World
/// [接收] 2024-01-01 12:00:01.456 (127.0.0.1:8080) Response data
/// ```
pub fn format_as_txt(messages: &[Message], view: MessageType) -> String {
    let mut output = String::new();

    for msg in messages {
        let direction = match msg.direction {
            MessageDirection::Sent => "发送",
            MessageDirection::Received => "接收",
        };

        let source_part = match &msg.source {
            Some(src) => format!(" ({})", src),
            None => String::new(),
        };

        output.push_str(&format!(
            "[{}] {}{} {}\n",
            direction,
            msg.timestamp,
            source_part,
            msg.content_for_view(view)
        ));
    }

    output
}

/// 将消息列表导出为 JSON 格式
///
/// `view` 同时决定 `content` 与 `type` 字段（保持一致：类型列就是导出视图）。
///
/// 格式示例：
/// ```json
/// [
///   {
///     "timestamp": "2024-01-01 12:00:00.123",
///     "direction": "sent",
///     "type": "text",
///     "source": null,
///     "content": "Hello World"
///   }
/// ]
/// ```
pub fn format_as_json(
    messages: &[Message],
    view: MessageType,
) -> Result<String, serde_json::Error> {
    #[derive(serde::Serialize)]
    struct ExportMessage {
        timestamp: String,
        direction: String,
        #[serde(rename = "type")]
        msg_type: String,
        source: Option<String>,
        content: String,
    }

    let export_messages: Vec<ExportMessage> = messages
        .iter()
        .map(|msg| ExportMessage {
            timestamp: msg.timestamp.clone(),
            direction: match msg.direction {
                MessageDirection::Sent => "sent".to_string(),
                MessageDirection::Received => "received".to_string(),
            },
            msg_type: match view {
                MessageType::Text => "text".to_string(),
                MessageType::Hex => "hex".to_string(),
            },
            source: msg.source.clone(),
            content: msg.content_for_view(view),
        })
        .collect();

    serde_json::to_string_pretty(&export_messages)
}

/// 将消息列表导出为 CSV 格式
///
/// 「类型」列与「内容」列统一按 `view` 输出，避免出现"类型=文本 但内容是十六进制"。
///
/// 格式示例：
/// ```csv
/// 时间戳,方向,类型,来源,内容
/// 2024-01-01 12:00:00.123,发送,文本,,Hello World
/// ```
pub fn format_as_csv(messages: &[Message], view: MessageType) -> String {
    let mut output = String::new();

    // CSV 表头
    output.push_str("时间戳,方向,类型,来源,内容\n");

    for msg in messages {
        let direction = match msg.direction {
            MessageDirection::Sent => "发送",
            MessageDirection::Received => "接收",
        };

        let msg_type = match view {
            MessageType::Text => "文本",
            MessageType::Hex => "十六进制",
        };

        let source = msg.source.as_deref().unwrap_or("");
        let content = msg.content_for_view(view);

        output.push_str(&format!(
            "{},{},{},{},{}\n",
            csv_escape(&msg.timestamp),
            csv_escape(direction),
            csv_escape(msg_type),
            csv_escape(source),
            csv_escape(&content),
        ));
    }

    output
}

/// CSV 字段转义：如果包含逗号、引号或换行，则用双引号包裹
pub(crate) fn csv_escape(field: &str) -> String {
    if field.contains(',') || field.contains('"') || field.contains('\n') || field.contains('\r') {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

/// 根据格式导出消息列表为字符串
pub fn format_messages(
    messages: &[Message],
    format: ExportFormat,
    view: MessageType,
) -> Result<String, String> {
    if messages.is_empty() {
        return Err("没有可导出的消息记录".to_string());
    }

    match format {
        ExportFormat::Txt => Ok(format_as_txt(messages, view)),
        ExportFormat::Json => {
            format_as_json(messages, view).map_err(|e| format!("JSON 序列化失败: {}", e))
        }
        ExportFormat::Csv => Ok(format_as_csv(messages, view)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_message(
        direction: MessageDirection,
        data: &[u8],
        msg_type: MessageType,
    ) -> Message {
        Message::new(direction, data.to_vec(), msg_type)
    }

    #[test]
    fn test_format_txt() {
        let messages = vec![
            create_test_message(MessageDirection::Sent, b"Hello", MessageType::Text),
            create_test_message(MessageDirection::Received, b"World", MessageType::Text),
        ];

        let result = format_as_txt(&messages, MessageType::Text);
        assert!(result.contains("[发送]"));
        assert!(result.contains("[接收]"));
        assert!(result.contains("Hello"));
        assert!(result.contains("World"));
    }

    #[test]
    /// 导出跟随显示视图：文本模式发来的消息也能按十六进制导出
    fn test_format_txt_follows_hex_view() {
        let messages = vec![create_test_message(
            MessageDirection::Received,
            b"Hello",
            MessageType::Text,
        )];

        let result = format_as_txt(&messages, MessageType::Hex);
        assert!(
            result.contains("48 65 6C 6C 6F"),
            "按十六进制导出: {result}"
        );
        assert!(!result.contains("Hello"));
    }

    #[test]
    fn test_format_json() {
        let messages = vec![create_test_message(
            MessageDirection::Sent,
            b"Hello",
            MessageType::Text,
        )];

        let result = format_as_json(&messages, MessageType::Text).unwrap();
        assert!(result.contains("\"direction\": \"sent\""));
        assert!(result.contains("\"type\": \"text\""));
        assert!(result.contains("\"content\": \"Hello\""));
    }

    #[test]
    /// JSON 导出的 type 字段与 content 口径一致（都跟视图走）
    fn test_format_json_hex_view_type_matches_content() {
        let messages = vec![create_test_message(
            MessageDirection::Sent,
            b"Hello",
            MessageType::Text,
        )];

        let result = format_as_json(&messages, MessageType::Hex).unwrap();
        assert!(result.contains("\"type\": \"hex\""));
        assert!(result.contains("\"content\": \"48 65 6C 6C 6F\""));
    }

    #[test]
    fn test_format_csv() {
        let messages = vec![
            create_test_message(MessageDirection::Sent, b"Hello", MessageType::Text),
            create_test_message(MessageDirection::Received, b"World", MessageType::Text),
        ];

        let result = format_as_csv(&messages, MessageType::Text);
        assert!(result.starts_with("时间戳,方向,类型,来源,内容\n"));
        assert!(result.contains("发送"));
        assert!(result.contains("接收"));
        assert!(result.contains("文本"));
        assert!(result.contains("Hello"));
        assert!(!result.contains("十六进制"));
    }

    #[test]
    /// CSV 的类型列与内容列按同一视图输出
    fn test_format_csv_hex_view() {
        let messages = vec![create_test_message(
            MessageDirection::Received,
            b"World",
            MessageType::Text,
        )];

        let result = format_as_csv(&messages, MessageType::Hex);
        assert!(result.contains("十六进制"));
        assert!(result.contains("57 6F 72 6C 64"));
        assert!(!result.contains("文本"));
    }

    #[test]
    fn test_csv_escape() {
        assert_eq!(csv_escape("simple"), "simple");
        assert_eq!(csv_escape("with,comma"), "\"with,comma\"");
        assert_eq!(csv_escape("with\"quote"), "\"with\"\"quote\"");
        assert_eq!(csv_escape("with\nnewline"), "\"with\nnewline\"");
    }

    #[test]
    fn test_format_empty_messages() {
        let result = format_messages(&[], ExportFormat::Txt, MessageType::Text);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "没有可导出的消息记录");
    }

    #[test]
    fn test_export_format_from_extension() {
        assert_eq!(
            ExportFormat::from_extension(Path::new("test.txt")),
            Some(ExportFormat::Txt)
        );
        assert_eq!(
            ExportFormat::from_extension(Path::new("test.JSON")),
            Some(ExportFormat::Json)
        );
        assert_eq!(
            ExportFormat::from_extension(Path::new("test.Csv")),
            Some(ExportFormat::Csv)
        );
        assert_eq!(
            ExportFormat::from_extension(Path::new("test.unknown")),
            None
        );
    }
}
