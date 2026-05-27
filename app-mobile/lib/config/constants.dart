/// App-wide constants
class AppConstants {
  AppConstants._();

  static const String appName = '无我笔记';
  static const String appVersion = '0.1.0';

  // Database
  static const String dbName = 'woo.db';
  static const int dbVersion = 1;

  // Sync
  static const int syncIntervalSeconds = 60;
  static const int cleanupSeconds = 7 * 24 * 60 * 60; // 7 days
  static const int tombstoneRetentionDays = 30;

  // Document
  static const int autoVersionMinChars = 100;
  static const int autoVersionMinLines = 10;
  static const int titleMaxLength = 40;
}
