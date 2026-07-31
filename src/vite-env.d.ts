/// <reference types="vite/client" />

/** 构建时由 vite.config.ts 从 package.json 注入 */
declare const __APP_VERSION__: string;
/** 未定稿大类功能「软件体检」的前端门控开关，由环境变量 DISKBUTLER_FEATURE_BLOATWARE 注入，默认 false */
declare const __FEATURE_BLOATWARE__: boolean;
