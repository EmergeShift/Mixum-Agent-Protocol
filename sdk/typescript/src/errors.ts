import { ERROR_CODES, type ErrorCode } from "./types.js";

export class MapError extends Error {
  readonly code: ErrorCode;
  readonly replyId?: string;

  constructor(code: ErrorCode, message: string, replyId?: string) {
    super(`[${code}] ${message}`);
    this.name = "MapError";
    this.code = code;
    this.replyId = replyId;
  }

  static isCode(value: string): value is ErrorCode {
    return (ERROR_CODES as readonly string[]).includes(value);
  }
}
