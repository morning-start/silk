//! 用户友好的错误信息转换层
//!
//! 将技术错误转换为用户友好的中文提示，隐藏技术细节。

use serde::{Deserialize, Serialize};

/// 用户友好的错误信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserFriendlyError {
    /// 错误标题
    pub title: String,
    /// 错误消息（用户友好）
    pub message: String,
    /// 建议操作
    pub suggestion: Option<String>,
    /// 错误类型（用于UI展示）
    pub error_type: UserFriendlyErrorType,
    /// 原始错误（仅用于日志，不展示给用户）
    #[serde(skip)]
    pub original_error: Option<String>,
}

/// 用户友好错误类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum UserFriendlyErrorType {
    /// 认证错误
    Authentication,
    /// 请求频率限制
    RateLimit,
    /// 服务不可用
    ServiceUnavailable,
    /// 请求格式错误
    BadRequest,
    /// 网络错误
    Network,
    /// 超时
    Timeout,
    /// 未知错误
    Unknown,
}

/// 将技术错误转换为用户友好信息
pub fn convert_error(status: u16, message: &str) -> UserFriendlyError {
    match status {
        401 | 403 => UserFriendlyError {
            title: "认证失败".to_string(),
            message: "AI服务认证失败，请检查您的API密钥是否正确".to_string(),
            suggestion: Some("请在设置中重新配置API密钥".to_string()),
            error_type: UserFriendlyErrorType::Authentication,
            original_error: Some(message.to_string()),
        },
        429 => UserFriendlyError {
            title: "请求过于频繁".to_string(),
            message: "AI服务请求过于频繁，请稍后再试".to_string(),
            suggestion: Some("您可以尝试减少请求频率或等待一段时间".to_string()),
            error_type: UserFriendlyErrorType::RateLimit,
            original_error: Some(message.to_string()),
        },
        500..=599 => UserFriendlyError {
            title: "服务暂时不可用".to_string(),
            message: "AI服务暂时不可用，请稍后再试".to_string(),
            suggestion: Some("请稍后重试，或联系服务提供商".to_string()),
            error_type: UserFriendlyErrorType::ServiceUnavailable,
            original_error: Some(message.to_string()),
        },
        408 => UserFriendlyError {
            title: "请求超时".to_string(),
            message: "请求处理超时，请稍后再试".to_string(),
            suggestion: Some("请检查网络连接，或稍后重试".to_string()),
            error_type: UserFriendlyErrorType::Timeout,
            original_error: Some(message.to_string()),
        },
        400 => UserFriendlyError {
            title: "请求格式错误".to_string(),
            message: "请求格式转换失败，请检查输入内容".to_string(),
            suggestion: Some("请确保输入符合AI服务的格式要求".to_string()),
            error_type: UserFriendlyErrorType::BadRequest,
            original_error: Some(message.to_string()),
        },
        _ => UserFriendlyError {
            title: "请求失败".to_string(),
            message: "AI服务出现问题，请稍后再试".to_string(),
            suggestion: None,
            error_type: UserFriendlyErrorType::Unknown,
            original_error: Some(format!("HTTP {status}: {message}")),
        },
    }
}

/// 从错误消息推断错误类型
pub fn infer_error_type(message: &str) -> UserFriendlyErrorType {
    let message_lower = message.to_lowercase();

    if message_lower.contains("api key")
        || message_lower.contains("api_key")
        || message_lower.contains("unauthorized")
        || message_lower.contains("authentication")
    {
        UserFriendlyErrorType::Authentication
    } else if message_lower.contains("rate limit")
        || message_lower.contains("too many requests")
        || message_lower.contains("429")
    {
        UserFriendlyErrorType::RateLimit
    } else if message_lower.contains("timeout") || message_lower.contains("timed out") {
        UserFriendlyErrorType::Timeout
    } else if message_lower.contains("network")
        || message_lower.contains("connection")
        || message_lower.contains("dns")
    {
        UserFriendlyErrorType::Network
    } else {
        UserFriendlyErrorType::Unknown
    }
}

/// 从错误消息创建用户友好错误
pub fn convert_from_message(message: &str) -> UserFriendlyError {
    let error_type = infer_error_type(message);

    match error_type {
        UserFriendlyErrorType::Authentication => UserFriendlyError {
            title: "认证失败".to_string(),
            message: "AI服务认证失败，请检查您的API密钥是否正确".to_string(),
            suggestion: Some("请在设置中重新配置API密钥".to_string()),
            error_type,
            original_error: Some(message.to_string()),
        },
        UserFriendlyErrorType::RateLimit => UserFriendlyError {
            title: "请求过于频繁".to_string(),
            message: "AI服务请求过于频繁，请稍后再试".to_string(),
            suggestion: Some("您可以尝试减少请求频率或等待一段时间".to_string()),
            error_type,
            original_error: Some(message.to_string()),
        },
        UserFriendlyErrorType::Timeout => UserFriendlyError {
            title: "请求超时".to_string(),
            message: "请求处理超时，请稍后再试".to_string(),
            suggestion: Some("请检查网络连接，或稍后重试".to_string()),
            error_type,
            original_error: Some(message.to_string()),
        },
        UserFriendlyErrorType::Network => UserFriendlyError {
            title: "网络错误".to_string(),
            message: "网络连接出现问题，请检查您的网络".to_string(),
            suggestion: Some("请检查网络连接后重试".to_string()),
            error_type,
            original_error: Some(message.to_string()),
        },
        _ => UserFriendlyError {
            title: "请求失败".to_string(),
            message: "AI服务出现问题，请稍后再试".to_string(),
            suggestion: Some("如问题持续，请联系技术支持".to_string()),
            error_type,
            original_error: Some(message.to_string()),
        },
    }
}

/// 错误转换服务
pub struct ErrorConverterService;

impl ErrorConverterService {
    /// 转换HTTP错误为用户友好信息
    pub fn convert_http_error(status: u16, message: &str) -> UserFriendlyError {
        convert_error(status, message)
    }

    /// 转换错误消息为用户友好信息
    pub fn convert_error_message(message: &str) -> UserFriendlyError {
        convert_from_message(message)
    }

    /// 转换错误为JSON格式（供前端使用）
    pub fn convert_to_json(status: u16, message: &str) -> serde_json::Value {
        let user_error = convert_error(status, message);
        serde_json::to_value(user_error).unwrap_or_else(|_| {
            serde_json::json!({
                "title": "系统错误",
                "message": "系统出现问题，请稍后再试",
                "error_type": "Unknown"
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_authentication_error() {
        let error = convert_error(401, "Invalid API key");
        assert_eq!(error.title, "认证失败");
        assert_eq!(error.error_type, UserFriendlyErrorType::Authentication);
        assert!(error.suggestion.is_some());
    }

    #[test]
    fn test_convert_forbidden_error() {
        let error = convert_error(403, "Forbidden");
        assert_eq!(error.title, "认证失败");
        assert_eq!(error.error_type, UserFriendlyErrorType::Authentication);
    }

    #[test]
    fn test_convert_rate_limit_error() {
        let error = convert_error(429, "Too many requests");
        assert_eq!(error.title, "请求过于频繁");
        assert_eq!(error.error_type, UserFriendlyErrorType::RateLimit);
    }

    #[test]
    fn test_convert_server_error() {
        let error = convert_error(500, "Internal server error");
        assert_eq!(error.title, "服务暂时不可用");
        assert_eq!(
            error.error_type,
            UserFriendlyErrorType::ServiceUnavailable
        );
    }

    #[test]
    fn test_convert_timeout_error() {
        let error = convert_error(408, "Request timeout");
        assert_eq!(error.title, "请求超时");
        assert_eq!(error.error_type, UserFriendlyErrorType::Timeout);
    }

    #[test]
    fn test_convert_bad_request_error() {
        let error = convert_error(400, "Bad request");
        assert_eq!(error.title, "请求格式错误");
        assert_eq!(error.error_type, UserFriendlyErrorType::BadRequest);
    }

    #[test]
    fn test_convert_unknown_error() {
        let error = convert_error(418, "I'm a teapot");
        assert_eq!(error.title, "请求失败");
        assert_eq!(error.error_type, UserFriendlyErrorType::Unknown);
    }

    #[test]
    fn test_infer_error_type_api_key() {
        assert_eq!(
            infer_error_type("Invalid API key"),
            UserFriendlyErrorType::Authentication
        );
        assert_eq!(
            infer_error_type("Unauthorized access"),
            UserFriendlyErrorType::Authentication
        );
    }

    #[test]
    fn test_infer_error_type_rate_limit() {
        assert_eq!(
            infer_error_type("Rate limit exceeded"),
            UserFriendlyErrorType::RateLimit
        );
        assert_eq!(
            infer_error_type("Too many requests"),
            UserFriendlyErrorType::RateLimit
        );
    }

    #[test]
    fn test_infer_error_type_timeout() {
        assert_eq!(
            infer_error_type("Request timed out"),
            UserFriendlyErrorType::Timeout
        );
    }

    #[test]
    fn test_infer_error_type_network() {
        assert_eq!(
            infer_error_type("Network connection failed"),
            UserFriendlyErrorType::Network
        );
    }

    #[test]
    fn test_convert_from_message() {
        let error = convert_from_message("Invalid API key provided");
        assert_eq!(error.title, "认证失败");
        assert_eq!(error.error_type, UserFriendlyErrorType::Authentication);
    }

    #[test]
    fn test_error_converter_service() {
        let error = ErrorConverterService::convert_http_error(401, "Unauthorized");
        assert_eq!(error.title, "认证失败");

        let json = ErrorConverterService::convert_to_json(429, "Rate limit");
        assert!(json.get("title").is_some());
        assert!(json.get("message").is_some());
    }
}
