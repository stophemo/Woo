declare interface Window {
  electronAPI: {
    minimize: () => void;
    maximize: () => void;
    close: () => void;
    openExternalLink: (url: string) => void;
    getAppVersion: () => Promise<string>;
  };
  // 本地版业务 IPC 入口：由 preload.cjs 通过 contextBridge 暴露
  woo: {
    invoke: <T = unknown>(channel: string, ...args: unknown[]) => Promise<{ ok: boolean; data?: T; message?: string }>;
  };
}
