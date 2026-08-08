/**
 * 错误信息转换服务
 *
 * 将技术错误转换为用户友好的中文提示。
 */

export interface UserFriendlyError {
  title: string;
  message: string;
  suggestion?: string;
  error_type:
    | 'Authentication'
    | 'RateLimit'
    | 'ServiceUnavailable'
    | 'BadRequest'
    | 'Network'
    | 'Timeout'
    | 'Unknown';
}

/**
 * 从HTTP状态码转换为用户友好错误
 */
export function convertHttpError(status: number, message?: string): UserFriendlyError {
  switch (status) {
    case 401:
    case 403:
      return {
        title: '认证失败',
        message: 'AI服务认证失败，请检查您的API密钥是否正确',
        suggestion: '请在设置中重新配置API密钥',
        error_type: 'Authentication',
      };
    case 429:
      return {
        title: '请求过于频繁',
        message: 'AI服务请求过于频繁，请稍后再试',
        suggestion: '您可以尝试减少请求频率或等待一段时间',
        error_type: 'RateLimit',
      };
    case 408:
      return {
        title: '请求超时',
        message: '请求处理超时，请稍后再试',
        suggestion: '请检查网络连接，或稍后重试',
        error_type: 'Timeout',
      };
    case 400:
      return {
        title: '请求格式错误',
        message: '请求格式转换失败，请检查输入内容',
        suggestion: '请确保输入符合AI服务的格式要求',
        error_type: 'BadRequest',
      };
    default:
      if (status >= 500) {
        return {
          title: '服务暂时不可用',
          message: 'AI服务暂时不可用，请稍后再试',
          suggestion: '请稍后重试，或联系服务提供商',
          error_type: 'ServiceUnavailable',
        };
      }
      return {
        title: '请求失败',
        message: message || 'AI服务出现问题，请稍后再试',
        suggestion: '如问题持续，请联系技术支持',
        error_type: 'Unknown',
      };
  }
}

/**
 * 从错误消息推断错误类型
 */
export function inferErrorType(message: string): UserFriendlyError['error_type'] {
  const lower = message.toLowerCase();

  if (
    lower.includes('api key') ||
    lower.includes('api_key') ||
    lower.includes('unauthorized') ||
    lower.includes('authentication') ||
    lower.includes('401')
  ) {
    return 'Authentication';
  }

  if (
    lower.includes('rate limit') ||
    lower.includes('too many requests') ||
    lower.includes('429')
  ) {
    return 'RateLimit';
  }

  if (lower.includes('timeout') || lower.includes('timed out') || lower.includes('408')) {
    return 'Timeout';
  }

  if (
    lower.includes('network') ||
    lower.includes('connection') ||
    lower.includes('dns') ||
    lower.includes('fetch failed')
  ) {
    return 'Network';
  }

  return 'Unknown';
}

/**
 * 从错误消息转换为用户友好错误
 */
export function convertErrorMessage(message: string): UserFriendlyError {
  const errorType = inferErrorType(message);

  switch (errorType) {
    case 'Authentication':
      return {
        title: '认证失败',
        message: 'AI服务认证失败，请检查您的API密钥是否正确',
        suggestion: '请在设置中重新配置API密钥',
        error_type: errorType,
      };
    case 'RateLimit':
      return {
        title: '请求过于频繁',
        message: 'AI服务请求过于频繁，请稍后再试',
        suggestion: '您可以尝试减少请求频率或等待一段时间',
        error_type: errorType,
      };
    case 'Timeout':
      return {
        title: '请求超时',
        message: '请求处理超时，请稍后再试',
        suggestion: '请检查网络连接，或稍后重试',
        error_type: errorType,
      };
    case 'Network':
      return {
        title: '网络错误',
        message: '网络连接出现问题，请检查您的网络',
        suggestion: '请检查网络连接后重试',
        error_type: errorType,
      };
    default:
      return {
        title: '请求失败',
        message: 'AI服务出现问题，请稍后再试',
        suggestion: '如问题持续，请联系技术支持',
        error_type: errorType,
      };
  }
}

/**
 * 从异常对象提取用户友好错误
 */
export function convertException(error: unknown): UserFriendlyError {
  if (typeof error === 'string') {
    return convertErrorMessage(error);
  }

  if (error instanceof Error) {
    return convertErrorMessage(error.message);
  }

  if (typeof error === 'object' && error !== null) {
    const obj = error as Record<string, unknown>;
    if (typeof obj.message === 'string') {
      return convertErrorMessage(obj.message);
    }
    if (typeof obj.status === 'number') {
      return convertHttpError(obj.status, String(obj.message));
    }
  }

  return {
    title: '未知错误',
    message: '系统出现问题，请稍后再试',
    suggestion: '如问题持续，请联系技术支持',
    error_type: 'Unknown',
  };
}

/**
 * 错误转换服务类
 */
export class ErrorConverterService {
  /**
   * 转换HTTP错误
   */
  static convertHttpError(status: number, message?: string): UserFriendlyError {
    return convertHttpError(status, message);
  }

  /**
   * 转换错误消息
   */
  static convertErrorMessage(message: string): UserFriendlyError {
    return convertErrorMessage(message);
  }

  /**
   * 转换异常对象
   */
  static convertException(error: unknown): UserFriendlyError {
    return convertException(error);
  }
}
