/// Uygulama genelinde kullanılan yapılandırma ayarları
class AppConfig {
  // Private constructor - singleton pattern
  AppConfig._();

  /// API Base URL
  static const String apiBaseUrl = 'https://one.web.tr';

  /// API Endpoint
  static const String apiEndpoint = '$apiBaseUrl/api';

  /// Media/Asset Base URL (görseller için)
  static const String mediaBaseUrl = apiBaseUrl;

  /// API Timeout (saniye)
  static const int apiTimeout = 30;

  /// Sayfa başına gösterilecek ürün sayısı
  static const int productsPerPage = 20;

  /// Varsayılan dil
  static const String defaultLanguage = 'tr';

  /// Desteklenen diller
  static const List<String> supportedLanguages = ['tr', 'en'];

  /// Debug mode
  static const bool isDebugMode = true;

  /// OpenRouteService API Key
  static const String openRouteServiceApiKey =
      '5b3ce3597851110001cf62482110982a52724a56b7294c7f59dfa3d7';

  /// OpenRouteService Base URL
  static const String openRouteServiceBaseUrl =
      'https://api.openrouteservice.org';

  /// WebSocket Base URL
  static const String wsBaseUrl = 'wss://one.web.tr';
}
