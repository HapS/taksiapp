import 'dart:convert';
import 'dart:math' as math;
import 'package:flutter/foundation.dart';
import 'package:http/http.dart' as http;
import 'package:latlong2/latlong.dart';
import '../../../core/config/app_config.dart';
import '../../auth/services/auth_service.dart';

/// Arama sonucu (autocomplete için).
///
/// DB veya Nominatim API'den gelen her bir sonucu temsil eder.
/// displayName: kullanıcıya gösterilen adres metni
/// coordinate: haritada gösterilecek LatLng koordinatı
/// source: "db" (kaydedilmiş mekan) veya "nominatim" (harita servisi)
/// id: DB'den geliyorsa mekan ID'si, değilse null
class SearchResult {
  final String displayName;
  final LatLng coordinate;
  final String source;
  final int? id;

  SearchResult({
    required this.displayName,
    required this.coordinate,
    this.source = 'nominatim',
    this.id,
  });
}

/// Ücret detayı — backend'den gelir, mobil client hesaplama yapmaz.
///
/// Backend kaynak: GET /api/ride/route → fare_info bloğu
/// Yolcu: rota seçildiği anda gösterilir (taksi çağırmadan önce)
/// Sürücü: teklif anında WS ride_offer mesajında gelir
class FareInfo {
  final double openingFee;
  final double minFare;
  final double perKmFee;
  final double estimatedFare;
  final String currency;

  FareInfo({
    required this.openingFee,
    required this.minFare,
    required this.perKmFee,
    required this.estimatedFare,
    required this.currency,
  });

  factory FareInfo.fromJson(Map<String, dynamic> json) => FareInfo(
        openingFee: (json['opening_fee'] as num).toDouble(),
        minFare: (json['min_fare'] as num).toDouble(),
        perKmFee: (json['per_km_fee'] as num).toDouble(),
        estimatedFare: (json['estimated_fare'] as num).toDouble(),
        currency: json['currency'] as String? ?? 'TRY',
      );

  /// Tahmini ücret — "₺25,00" formatında
  String get formattedEstimate => '₺${estimatedFare.toStringAsFixed(2)}';

  /// Açılış ücreti — "₺15,00"
  String get formattedOpeningFee => '₺${openingFee.toStringAsFixed(2)}';

  /// Km başına — "₺8,00/km"
  String get formattedPerKm => '₺${perKmFee.toStringAsFixed(2)}/km';

  /// Minimum ücret — "₺25,00"
  String get formattedMinFare => '₺${minFare.toStringAsFixed(2)}';

  Map<String, dynamic> toJson() => {
    'opening_fee': openingFee,
    'min_fare': minFare,
    'per_km_fee': perKmFee,
    'estimated_fare': estimatedFare,
    'currency': currency,
  };
}

/// Rota bilgisi — mesafe ve süre dahil.
///
/// ORS/OSRM backend proxy'sinden gelen rota verisini temsil eder.
/// points: haritada çizilecek rota noktaları (LatLng listesi)
/// distanceKm: toplam mesafe (km)
/// durationSeconds: tahmini süre (saniye)
/// fareInfo: backend'den gelen ücret detayı (null olabilir — fallback durumunda)
class RouteInfo {
  final List<LatLng> points;
  final double distanceKm;
  final int durationSeconds;
  final FareInfo? fareInfo;

  RouteInfo({
    required this.points,
    required this.distanceKm,
    required this.durationSeconds,
    this.fareInfo,
  });

  /// Mesafeyi insan-readable formata çevirir.
  /// 1km altındaysa metre, üstündeyse km cinsinden gösterir.
  String get formattedDistance {
    if (distanceKm < 1) {
      return '${(distanceKm * 1000).round()} m';
    }
    return '${distanceKm.toStringAsFixed(1)} km';
  }

  /// Süreyi insan-readable formata çevirir.
  /// 1dk altı → saniye, 1sa altı → dakika, üstü → saat+dakika
  String get formattedDuration {
    if (durationSeconds < 60) {
      return '${durationSeconds}s';
    }
    final minutes = durationSeconds ~/ 60;
    if (minutes < 60) {
      return '$minutes dk';
    }
    final hours = minutes ~/ 60;
    final remainingMinutes = minutes % 60;
    return '$hours s $remainingMinutes dk';
  }
}

/// OpenRouteService API işlemleri.
///
/// Bu sınıf backend proxy'siz doğrudan ORS API'ye istek atar.
/// Backend bu verileri kullanmaz; yalnızca client tarafında rota çizgisi
/// gösterimi ve ETA hesaplaması için kullanılır.
///
/// Backend de rota hesaplaması yapar (ORS proxy endpoint'i),
/// ancak mobil şu an doğrudan ORS'ye bağlanmaktadır.
/// TODO: RouteService.getRoute() backend proxy'sine yönlendirilecek.
class RouteService {
  static const String _baseUrl = AppConfig.openRouteServiceBaseUrl;
  static const String _apiKey = AppConfig.openRouteServiceApiKey;

  /// Adres araması (autocomplete).
  ///
  /// ORS Geocode Autocomplete API'ye istek atar.
  /// [query] en az 3 karakter olmalı (daha kısa ise boş liste döner).
  ///
  /// Backend ile ilişkisi yoktur; üçüncü taraf ORS API'ye doğrudan erişir.
  /// Kullanıcı arama kutusuna yazdıkça debounce ile çağrılır.
  static Future<List<SearchResult>> searchAddress(String query) async {
    if (query.length < 3) return [];

    final encodedQuery = Uri.encodeComponent(query);
    final url = Uri.parse(
      '$_baseUrl/geocode/autocomplete?api_key=$_apiKey&text=$encodedQuery',
    );

    try {
      final response = await http.get(url);

      if (response.statusCode == 200) {
        final data = json.decode(response.body);
        final features = data['features'] as List;

        return features.map((f) {
          final coords = f['geometry']['coordinates'];
          return SearchResult(
            displayName: f['properties']['label'],
            coordinate: LatLng(coords[1], coords[0]),
          );
        }).toList();
      } else {
        return [];
      }
    } catch (e) {
      return [];
    }
  }

  /// İki nokta arasında araç rotası hesaplar.
  ///
  /// Backend proxy endpoint'ini kullanır: GET /api/ride/route
  /// Backend ORS → OSRM → düz çizgi fallback yapar.
  /// [start] başlangıç noktası
  /// [end] hedef noktası
  static Future<RouteInfo> getRoute(LatLng start, LatLng end) async {
    final url = Uri.parse(
      '${AppConfig.apiEndpoint}/ride/route'
      '?start_lat=${start.latitude}'
      '&start_lon=${start.longitude}'
      '&end_lat=${end.latitude}'
      '&end_lon=${end.longitude}',
    );

    try {
      final authService = AuthService();
      final token = await authService.getAccessToken();

      final response = await http.get(
        url,
        headers: {
          'Accept': 'application/json',
          if (token != null) 'Authorization': 'Bearer $token',
        },
      );

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body) as Map<String, dynamic>;
        final points = data['data'] as List?;

        if (points != null && points.length >= 2) {
          final latLngPoints = points
              .map((p) => LatLng(
                    (p as Map)['lat'] as double,
                    p['lon'] as double,
                  ))
              .toList();

          final distanceKm = (data['distance_km'] as num?)?.toDouble()
              ?? _haversineKm(start, end);
          final durationSec = (data['duration_sec'] as num?)?.toInt()
              ?? (_haversineKm(start, end) / 30 * 3600).round();

          // Backend'den gelen ücret bilgisi
          final fareInfoJson = data['fare_info'] as Map<String, dynamic>?;
          final fareInfo = fareInfoJson != null ? FareInfo.fromJson(fareInfoJson) : null;

          return RouteInfo(
            points: latLngPoints,
            distanceKm: distanceKm,
            durationSeconds: durationSec,
            fareInfo: fareInfo,
          );
        }
      }

      // Fallback: düz çizgi
      return RouteInfo(
        points: [start, end],
        distanceKm: _haversineKm(start, end),
        durationSeconds: (_haversineKm(start, end) / 30 * 3600).round(),
      );
    } catch (e) {
      debugPrint('RouteService.getRoute error: $e');
      return RouteInfo(
        points: [start, end],
        distanceKm: _haversineKm(start, end),
        durationSeconds: (_haversineKm(start, end) / 30 * 3600).round(),
      );
    }
  }

  /// Haversine formülü ile iki nokta arası mesafe (km)
  static double _haversineKm(LatLng a, LatLng b) {
    const r = 6371.0;
    final dLat = _toRad(b.latitude - a.latitude);
    final dLon = _toRad(b.longitude - a.longitude);
    final h = math.sin(dLat / 2) * math.sin(dLat / 2) +
        math.cos(_toRad(a.latitude)) *
            math.cos(_toRad(b.latitude)) *
            math.sin(dLon / 2) *
            math.sin(dLon / 2);
    return r * 2 * math.atan2(math.sqrt(h), math.sqrt(1 - h));
  }

  static double _toRad(double deg) => deg * math.pi / 180;
}