import 'package:flutter/material.dart';

/// Cache yönetimi için yardımcı sınıf
class CacheManager {
  CacheManager._();

  /// Tüm image cache'ini temizle
  static void clearImageCache() {
    imageCache.clear();
    imageCache.clearLiveImages();
    debugPrint('🗑️ Image cache temizlendi');
  }

  /// Belirli bir image'ın cache'ini temizle
  static void evictImage(String url) {
    imageCache.evict(url);
    debugPrint('🗑️ Image cache temizlendi: $url');
  }
}
