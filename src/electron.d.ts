declare interface Window {
  electronAPI: {
    minimize: () => void;
    maximize: () => void;
    close: () => void;
    openExternalLink: (url: string) => void;
    getAppVersion: () => Promise<string>;
  };
}