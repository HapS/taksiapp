import 'dart:convert';
import 'package:flutter/foundation.dart';
import 'package:http/http.dart' as http;
import 'package:latlong2/latlong.dart';
import '../../../core/config/app_config.dart';
import '../../auth/services/auth_service.dart';
import 'route_service.dart' show FareInfo, RouteService, SearchResult;

/// Debug log helper — istek ve yanıtları konsola yazdırır.
void _logHttp({
  required String method,
  required String url,
  int? statusCode,
  Map<String, dynamic>? requestBody,
  Map<String, dynamic>? responseBody,
  Object? error,
}) {
  final buf = StringBuffer();
  buf.writeln('══════ HTTP $method ══════');
  buf.writeln('URL: $url');
  if (requestBody != null) buf.writeln('GÖNDERİLEN: $requestBody');
  if (statusCode != null) buf.writeln('DURUM: $statusCode');
  if (responseBody != null) buf.writeln('YANIT: $responseBody');
  if (error != null) buf.writeln('HATA: $error');
  buf.write('══════════════════════════');
  debugPrint(buf.toString());
}

/// Sürücü bilgi modeli.
///
/// Backend GET /api/ride/:id yanıtındaki driver objesinden deserialize edilir.
/// Backend kaynak: rides tablosu → drivers tablosu JOIN
/// Alanlar: full_name, vehicle_plate, vehicle_model, phone
class DriverInfo {
  final String fullName;
  final String vehiclePlate;
  final String vehicleModel;
  final String phone;

  DriverInfo({
    required this.fullName,
    required this.vehiclePlate,
    required this.vehicleModel,
    required this.phone,
  });

  /// Backend JSON'ından DriverInfo oluştur.
  /// Backend controller (ride.rs -> RideDetail) driver alanı bu yapıdadır.
  factory DriverInfo.fromJson(Map<String, dynamic> json) => DriverInfo(
        fullName: json['full_name'] as String,
        vehiclePlate: json['vehicle_plate'] as String,
        vehicleModel: json['vehicle_model'] as String,
        phone: json['phone'] as String,
      );
}

/// Yakındaki müsait sürücü modeli.
/// GET /api/ride/drivers/nearby yanıtından deserialize edilir.
class NearbyDriver {
  final int id;
  final double lat;
  final double lon;
  final String vehicleModel;
  final String vehiclePlate;
  final double rating;
  final bool isOnRide;

  NearbyDriver({
    required this.id,
    required this.lat,
    required this.lon,
    required this.vehicleModel,
    required this.vehiclePlate,
    required this.rating,
    required this.isOnRide,
  });

  factory NearbyDriver.fromJson(Map<String, dynamic> json) => NearbyDriver(
        id: json['id'] as int,
        lat: (json['current_lat'] as num).toDouble(),
        lon: (json['current_lon'] as num).toDouble(),
        vehicleModel: json['vehicle_model'] as String,
        vehiclePlate: json['vehicle_plate'] as String,
        rating: (json['rating'] as num).toDouble(),
        isOnRide: json['is_on_ride'] as bool,
      );
}

/// POST /api/ride/request yanıt modeli.
class RideRequestResponse {
  final bool success;
  final int? rideId;
  final String? status;
  final String? error;

  RideRequestResponse({
    required this.success,
    this.rideId,
    this.status,
    this.error,
  });
}

/// GET /api/ride/:id yanıt modeli.
///
/// Backend controller (ride.rs -> RideDetail) tarafından doldurulur.
/// Polling ile her 5 saniyede bir çağrılır ve rideProvider state'ini günceller.
///
/// Alanlar:
/// - status: ride durumu (idle, searching, accepted, picked_up, completed, no_driver, cancelled)
/// - driver: sürücü bilgisi (accepted/picked_up durumunda dolu)
/// - distanceKm/durationSec: rota mesafe/süre (backend ORS hesaplaması)
/// - fareAmount: ücret (backend fare hesaplaması)
/// - driverLat/driverLon: sürücü anlık konumu (WS fallback)
class RideStatusResponse {
  final bool success;
  final int? id;
  final String? status;
  final DriverInfo? driver;
  final double? distanceKm;
  final int? durationSec;
  final double? fareAmount;
  final FareInfo? fareInfo;
  final double? driverLat;
  final double? driverLon;
  final String? error;

  RideStatusResponse({
    required this.success,
    this.id,
    this.status,
    this.driver,
    this.distanceKm,
    this.durationSec,
    this.fareAmount,
    this.fareInfo,
    this.driverLat,
    this.driverLon,
    this.error,
  });
}

/// Aktif yolculuk sorgulama yanıt modeli.
///
/// Backend: GET /api/ride/driver/active
/// Sürücünün yarım kalmış (accepted/picked_up) yolculuğunu döndürür.
class ActiveRideInfo {
  final int rideId;
  final String status;
  final String pickupAddress;
  final String dropoffAddress;
  final double pickupLat;
  final double pickupLon;
  final double dropoffLat;
  final double dropoffLon;
  final double? distanceKm;
  final int? durationSec;
  final double? fareAmount;
  final FareInfo? fareInfo;

  ActiveRideInfo({
    required this.rideId,
    required this.status,
    required this.pickupAddress,
    required this.dropoffAddress,
    required this.pickupLat,
    required this.pickupLon,
    required this.dropoffLat,
    required this.dropoffLon,
    this.distanceKm,
    this.durationSec,
    this.fareAmount,
    this.fareInfo,
  });

  factory ActiveRideInfo.fromJson(Map<String, dynamic> json) {
    return ActiveRideInfo(
      rideId: json['ride_id'] as int,
      status: json['status'] as String,
      pickupAddress: json['pickup_address'] as String,
      dropoffAddress: json['dropoff_address'] as String,
      pickupLat: (json['pickup_lat'] as num).toDouble(),
      pickupLon: (json['pickup_lon'] as num).toDouble(),
      dropoffLat: (json['dropoff_lat'] as num).toDouble(),
      dropoffLon: (json['dropoff_lon'] as num).toDouble(),
      distanceKm: (json['distance_km'] as num?)?.toDouble(),
      durationSec: json['duration_sec'] as int?,
      fareAmount: (json['fare_amount'] as num?)?.toDouble(),
      fareInfo: json['fare_info'] != null ? FareInfo.fromJson(json['fare_info'] as Map<String, dynamic>) : null,
    );
  }
}

/// Backend ride API istemcisi.
///
/// Tüm HTTP istekleri bu sınıf üzerinden geçer.
/// AuthService'den JWT token alır ve Authorization header'ına ekler.
///
/// Backend endpoint'leri:
/// - POST /api/ride/request    → Yeni yolculuk talebi oluştur
/// - GET  /api/ride/:id        → Yolculuk durumu sorgula (polling)
/// - POST /api/ride/:id/cancel → Yolculuğu iptal et
/// - POST /api/ride/:id/status → Yolculuk durumunu güncelle (picked_up, completed)
class RideService {
  static final _authService = AuthService();

  /*
  // Artık kullanılmıyor — WS bağlantısı ve request_ride JWT token ile yapılıyor.
  // Backend user_id'yi JWT Claims'den çıkarır.
  static int? parseUserIdFromToken(String token) {
    try {
      final parts = token.split('.');
      if (parts.length != 3) return null;
      final normalized = base64Url.normalize(parts[1]);
      final decoded = utf8.decode(base64Url.decode(normalized));
      final payload = json.decode(decoded) as Map<String, dynamic>;
      final id = payload['user_id'] ?? payload['sub'];
      if (id is int) return id;
      if (id is String) return int.tryParse(id);
      return null;
    } catch (e) {
      debugPrint('RideService: JWT parse error: $e');
      return null;
    }
  }
  */

  /// Yeni yolculuk talebi oluştur.
  ///
  /// Backend: POST /api/ride/request (JWT Auth gerekli)
  /// Headers: Authorization: Bearer `<token>`
  /// Body: { pickup_lat, pickup_lon, pickup_address, dropoff_lat, dropoff_lon, dropoff_address }
  /// Yanıt: { ride_id, status }
  ///
  /// user_id JWT token'dan backend tarafından çıkarılır, body'ye eklenmez.
  /// Backend yeni bir ride kaydı oluşturur (status: pending),
  /// ardından yakındaki online sürücülere WS üzerinden offer gönderir.
  static Future<RideRequestResponse> requestRide({
    required double pickupLat,
    required double pickupLon,
    required String pickupAddress,
    required double dropoffLat,
    required double dropoffLon,
    required String dropoffAddress,
  }) async {
    try {
      final token = await _authService.getAccessToken();
      if (token == null) {
        return RideRequestResponse(success: false, error: 'Token bulunamadı');
      }

      final response = await http.post(
        Uri.parse('${AppConfig.apiEndpoint}/ride/request'),
        headers: {
          'Content-Type': 'application/json',
          'Authorization': 'Bearer $token',
        },
        body: jsonEncode({
          'pickup_lat': pickupLat,
          'pickup_lon': pickupLon,
          'pickup_address': pickupAddress,
          'dropoff_lat': dropoffLat,
          'dropoff_lon': dropoffLon,
          'dropoff_address': dropoffAddress,
        }),
      );

      _logHttp(
        method: 'POST',
        url: '/ride/request',
        statusCode: response.statusCode,
        requestBody: {
          'pickup_lat': pickupLat, 'pickup_lon': pickupLon,
          'dropoff_lat': dropoffLat, 'dropoff_lon': dropoffLon,
        },
        responseBody: response.statusCode == 200 || response.statusCode == 201
            ? jsonDecode(response.body) as Map<String, dynamic>
            : null,
      );

      if (response.statusCode == 200 || response.statusCode == 201) {
        final data = jsonDecode(response.body) as Map<String, dynamic>;
        return RideRequestResponse(
          success: true,
          rideId: data['ride_id'] as int?,
          status: data['status'] as String?,
        );
      } else {
        final data = jsonDecode(response.body) as Map<String, dynamic>;
        return RideRequestResponse(
          success: false,
          error: data['message'] as String? ?? 'Sunucu hatası',
        );
      }
    } catch (e) {
      debugPrint('RideService: requestRide error: $e');
      return RideRequestResponse(success: false, error: e.toString());
    }
  }

  /// Yolculuk durumunu sorgula (polling).
  ///
  /// Backend: GET /api/ride/:id
  /// Headers: Authorization: Bearer `<token>`
  /// Yanıt: { id, status, driver{...}, pickup_lat, pickup_lon, dropoff_lat, dropoff_lon,
  ///          distance_km, duration_sec, fare_amount }
  ///
  /// 5 saniyede bir çağrılır (_startPolling).
  /// WS kesintilerinde fallback olarak çalışır.
  /// driver_lat/driver_lon alanları sürücü anlık konumunu taşır (WS driver_location fallback).
  static Future<RideStatusResponse> getRideStatus(int rideId) async {
    try {
      final token = await _authService.getAccessToken();
      if (token == null) {
        return RideStatusResponse(success: false, error: 'Token bulunamadı');
      }

      final response = await http.get(
        Uri.parse('${AppConfig.apiEndpoint}/ride/$rideId'),
        headers: {
          'Content-Type': 'application/json',
          'Authorization': 'Bearer $token',
        },
      );

      _logHttp(
        method: 'GET',
        url: '/ride/$rideId',
        statusCode: response.statusCode,
        responseBody: response.statusCode == 200
            ? jsonDecode(response.body) as Map<String, dynamic>
            : null,
      );

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body) as Map<String, dynamic>;
        final driverData = data['driver'] as Map<String, dynamic>?;
        final fareInfoJson = data['fare_info'] as Map<String, dynamic>?;
        return RideStatusResponse(
          success: true,
          id: data['id'] as int?,
          status: data['status'] as String?,
          driver: driverData != null ? DriverInfo.fromJson(driverData) : null,
          distanceKm: (data['distance_km'] as num?)?.toDouble(),
          durationSec: data['duration_sec'] as int?,
          fareAmount: (data['fare_amount'] as num?)?.toDouble(),
          fareInfo: fareInfoJson != null ? FareInfo.fromJson(fareInfoJson) : null,
          driverLat: (driverData?['current_lat'] as num?)?.toDouble(),
          driverLon: (driverData?['current_lon'] as num?)?.toDouble(),
        );
      } else {
        final data = jsonDecode(response.body) as Map<String, dynamic>;
        return RideStatusResponse(
          success: false,
          error: data['message'] as String? ?? 'Sunucu hatası',
        );
      }
    } catch (e) {
      debugPrint('RideService: getRideStatus error: $e');
      return RideStatusResponse(success: false, error: e.toString());
    }
  }

  /// Yolculuğu iptal et.
  ///
  /// Backend: POST /api/ride/:id/cancel
  /// Headers: Authorization: Bearer `<token>`
  /// Body: { by: "passenger" | "driver" }
  ///
  /// Backend ridestatus'unu "cancelled" olarak günceller,
  /// WS üzerinden ride_status_changed mesajı yayınlar,
  /// ride_rooms'dan kaydı kaldırır.
  static Future<bool> cancelRide(int rideId, {String by = 'driver'}) async {
    try {
      final token = await _authService.getAccessToken();
      if (token == null) return false;

      final response = await http.post(
        Uri.parse('${AppConfig.apiEndpoint}/ride/$rideId/cancel'),
        headers: {
          'Content-Type': 'application/json',
          'Authorization': 'Bearer $token',
        },
        body: jsonEncode({'by': by}),
      );

      _logHttp(
        method: 'POST',
        url: '/ride/$rideId/cancel',
        statusCode: response.statusCode,
        requestBody: {'by': by},
      );

      return response.statusCode == 200;
    } catch (e) {
      debugPrint('RideService: cancelRide error: $e');
      return false;
    }
  }

  /// Yolculuk durumunu güncelle (picked_up veya completed).
  ///
  /// Backend: POST /api/ride/:id/status
  /// Headers: Authorization: Bearer `<token>`
  /// Body: { status: "picked_up" | "completed" }
  ///
  /// Backend:
  /// - picked_up: ride status → picked_up, WS broadcast (ride_status_changed)
  /// - completed: ride status → completed, WS broadcast, ride_rooms temizleme
  ///
  /// Sürücü (bot) tarafından da çağrılır (fake_driver_bot.py).
  static Future<bool> updateRideStatus(int rideId, String status) async {
    try {
      final token = await _authService.getAccessToken();
      if (token == null) return false;

      final response = await http.post(
        Uri.parse('${AppConfig.apiEndpoint}/ride/$rideId/status'),
        headers: {
          'Content-Type': 'application/json',
          'Authorization': 'Bearer $token',
        },
        body: jsonEncode({'status': status}),
      );

      _logHttp(
        method: 'POST',
        url: '/ride/$rideId/status',
        statusCode: response.statusCode,
        requestBody: {'status': status},
      );

      return response.statusCode == 200;
    } catch (e) {
      debugPrint('RideService: updateRideStatus error: $e');
      return false;
    }
  }

  /// Sürücünün aktif (accepted/picked_up) yolculuğunu döndürür.
  /// Yoksa null döner.
  /// Backend: GET /api/ride/driver/active (JWT Auth gerekli)
  static Future<ActiveRideInfo?> getDriverActiveRide() async {
    try {
      final token = await _authService.getAccessToken();
      if (token == null) return null;

      final url = Uri.parse('${AppConfig.apiEndpoint}/ride/driver/active');
      final response = await http.get(
        url,
        headers: {
          'Content-Type': 'application/json',
          'Authorization': 'Bearer $token',
        },
      );

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body) as Map<String, dynamic>;
        final activeRide = data['active_ride'];
        if (activeRide == null) return null;
        return ActiveRideInfo.fromJson(activeRide as Map<String, dynamic>);
      }
      return null;
    } catch (e) {
      debugPrint('RideService.getDriverActiveRide error: $e');
      return null;
    }
  }

  /// Yolcunun aktif (accepted/picked_up) yolculuğunu döndürür.
  /// Yoksa null döner.
  /// Backend: GET /api/ride/passenger/active (JWT Auth gerekli)
  static Future<ActiveRideInfo?> getPassengerActiveRide() async {
    try {
      final token = await _authService.getAccessToken();
      if (token == null) return null;

      final url = Uri.parse('${AppConfig.apiEndpoint}/ride/passenger/active');
      final response = await http.get(
        url,
        headers: {
          'Content-Type': 'application/json',
          'Authorization': 'Bearer $token',
        },
      );

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body) as Map<String, dynamic>;
        final activeRide = data['active_ride'];
        if (activeRide == null) return null;
        return ActiveRideInfo.fromJson(activeRide as Map<String, dynamic>);
      }
      return null;
    } catch (e) {
      debugPrint('RideService.getPassengerActiveRide error: $e');
      return null;
    }
  }

  /// Yakındaki müsait sürücüleri getirir.
  /// Görünen harita alanı içindeki online sürücüleri döndürür.
  static Future<List<NearbyDriver>> getNearbyDrivers({
    required double minLat,
    required double maxLat,
    required double minLon,
    required double maxLon,
    String status = 'available',
  }) async {
    try {
      final token = await _authService.getAccessToken();
      if (token == null) return [];

      final url = Uri.parse(
        '${AppConfig.apiEndpoint}/ride/drivers/nearby'
        '?min_lat=$minLat&max_lat=$maxLat&min_lon=$minLon&max_lon=$maxLon&status=$status',
      );
      final response = await http.get(
        url,
        headers: {
          'Content-Type': 'application/json',
          'Authorization': 'Bearer $token',
        },
      );

      _logHttp(method: 'GET', url: '/ride/drivers/nearby', statusCode: response.statusCode);

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body) as Map<String, dynamic>;
        final drivers = data['drivers'] as List<dynamic>;
        return drivers
            .map((d) => NearbyDriver.fromJson(d as Map<String, dynamic>))
            .toList();
      }
      return [];
    } catch (e) {
      debugPrint('RideService.getNearbyDrivers error: $e');
      return [];
    }
  }
}

// ---------------------------------------------------------------------------
// Geçmiş Yolculuklar
// ---------------------------------------------------------------------------

/// Backend: GET /api/ride/history yanıtındaki `counterparty` objesi.
///
/// Sürücü tarafı: yolcu bilgisi.
/// Yolcu tarafı: sürücü bilgisi (vehicle_model/plate dolu).
class HistoryCounterparty {
  final int userId;
  final String fullName;
  final String? phone;
  final String? vehicleModel;
  final String? vehiclePlate;

  HistoryCounterparty({
    required this.userId,
    required this.fullName,
    this.phone,
    this.vehicleModel,
    this.vehiclePlate,
  });

  factory HistoryCounterparty.fromJson(Map<String, dynamic> json) {
    return HistoryCounterparty(
      userId: (json['user_id'] as num?)?.toInt() ?? 0,
      fullName: (json['full_name'] as String?) ?? '-',
      phone: json['phone'] as String?,
      vehicleModel: json['vehicle_model'] as String?,
      vehiclePlate: json['vehicle_plate'] as String?,
    );
  }
}

/// Tek bir geçmiş yolculuk kaydı.
///
/// Backend: GET /api/ride/history yanıtındaki `rides[]` elemanı.
class RideHistoryItem {
  final int rideId;
  final String status;
  final String pickupAddress;
  final double pickupLat;
  final double pickupLon;
  final String dropoffAddress;
  final double dropoffLat;
  final double dropoffLon;
  final double? distanceKm;
  final int? durationSec;
  final double? fareAmount;
  final String requestedAt;
  final String? acceptedAt;
  final String? pickedUpAt;
  final String? completedAt;
  final String? cancelledAt;
  final HistoryCounterparty counterparty;

  RideHistoryItem({
    required this.rideId,
    required this.status,
    required this.pickupAddress,
    required this.pickupLat,
    required this.pickupLon,
    required this.dropoffAddress,
    required this.dropoffLat,
    required this.dropoffLon,
    this.distanceKm,
    this.durationSec,
    this.fareAmount,
    required this.requestedAt,
    this.acceptedAt,
    this.pickedUpAt,
    this.completedAt,
    this.cancelledAt,
    required this.counterparty,
  });

  factory RideHistoryItem.fromJson(Map<String, dynamic> json) {
    return RideHistoryItem(
      rideId: (json['ride_id'] as num).toInt(),
      status: json['status'] as String,
      pickupAddress: json['pickup_address'] as String,
      pickupLat: (json['pickup_lat'] as num).toDouble(),
      pickupLon: (json['pickup_lon'] as num).toDouble(),
      dropoffAddress: json['dropoff_address'] as String,
      dropoffLat: (json['dropoff_lat'] as num).toDouble(),
      dropoffLon: (json['dropoff_lon'] as num).toDouble(),
      distanceKm: (json['distance_km'] as num?)?.toDouble(),
      durationSec: (json['duration_sec'] as num?)?.toInt(),
      fareAmount: (json['fare_amount'] as num?)?.toDouble(),
      requestedAt: json['requested_at'] as String,
      acceptedAt: json['accepted_at'] as String?,
      pickedUpAt: json['picked_up_at'] as String?,
      completedAt: json['completed_at'] as String?,
      cancelledAt: json['cancelled_at'] as String?,
      counterparty: HistoryCounterparty.fromJson(
        json['counterparty'] as Map<String, dynamic>,
      ),
    );
  }
}

/// Geçmiş yolculuk sayfası yanıtı — cursor-based pagination destekli.
///
/// Backend: GET /api/ride/history
class RideHistoryPage {
  final String role;
  final int limit;
  final int count;
  final bool hasMore;
  final String? nextCursor;
  final int total;
  final List<RideHistoryItem> rides;

  RideHistoryPage({
    required this.role,
    required this.limit,
    required this.count,
    required this.hasMore,
    this.nextCursor,
    required this.total,
    required this.rides,
  });

  factory RideHistoryPage.fromJson(Map<String, dynamic> json) {
    return RideHistoryPage(
      role: (json['role'] as String?) ?? 'passenger',
      limit: (json['limit'] as num?)?.toInt() ?? 20,
      count: (json['count'] as num?)?.toInt() ?? 0,
      hasMore: (json['has_more'] as bool?) ?? false,
      nextCursor: json['next_cursor'] as String?,
      total: (json['total'] as num?)?.toInt() ?? 0,
      rides: (json['rides'] as List<dynamic>)
          .map((r) => RideHistoryItem.fromJson(r as Map<String, dynamic>))
          .toList(),
    );
  }
}

/// Geçmiş yolculuk API istemcisi.
///
/// Backend: GET /api/ride/history (JWT Auth gerekli)
/// Cursor-based pagination kullanır.
class RideHistoryApi {
  static final AuthService _auth = AuthService();

  /// Geçmiş yolculukları cursor-based pagination ile getirir.
  ///
  /// Parametreler:
  /// - [status]: Opsiyonel filtre (`completed` | `cancelled` | `no_driver`).
  ///   null ise tüm bitmiş yolculuklar.
  /// - [role]: `auto` (default) | `driver` | `passenger`.
  /// - [cursor]: İlk sayfa için null; sonraki sayfa için response'tan dönen `nextCursor`.
  /// - [limit]: Sayfa başına kayıt (default 20).
  static Future<RideHistoryPage> getRideHistory({
    String? status,
    String role = 'auto',
    String? cursor,
    int limit = 20,
  }) async {
    try {
      final token = await _auth.getAccessToken();
      if (token == null) {
        return _empty(role, limit);
      }

      final queryParts = <String>[
        'role=$role',
        'limit=$limit',
      ];
      if (status != null && status.isNotEmpty) {
        queryParts.add('status=$status');
      }
      if (cursor != null && cursor.isNotEmpty) {
        queryParts.add('cursor=$cursor');
      }
      final url = Uri.parse(
        '${AppConfig.apiEndpoint}/ride/history?${queryParts.join('&')}',
      );

      final response = await http.get(
        url,
        headers: {
          'Content-Type': 'application/json',
          'Authorization': 'Bearer $token',
        },
      );

      _logHttp(
        method: 'GET',
        url: '/ride/history?status=$status&cursor=${cursor != null ? "***" : "null"}',
        statusCode: response.statusCode,
      );

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body) as Map<String, dynamic>;
        return RideHistoryPage.fromJson(data);
      }
      debugPrint('RideHistoryApi.getRideHistory: HTTP ${response.statusCode}');
      return _empty(role, limit);
    } catch (e) {
      debugPrint('RideHistoryApi.getRideHistory error: $e');
      return _empty(role, limit);
    }
  }

  static RideHistoryPage _empty(String role, int limit) {
    return RideHistoryPage(
      role: role == 'auto' ? 'passenger' : role,
      limit: limit,
      count: 0,
      hasMore: false,
      total: 0,
      rides: [],
    );
  }
}

/// Konum arama istemcisi.
///
/// Arama stratejisi:
/// 1. Önce backend DB'de ara → /api/locations/search
/// 2. DB'den yeterli sonuç gelmezse RouteService.searchAddress() ile ORS'ye fallback
/// 3. Sonuçlar birleştirilir, DB sonuçları önce listelenir
class LocationSearchApi {
  static final AuthService _auth = AuthService();

  /// Konum arama — önce DB, sonra Nominatim fallback.
  ///
  /// [query] en az 2 karakter olmalı.
  /// [limit] toplam maksimum sonuç sayısı (default 10).
  static Future<List<SearchResult>> search(String query, {int limit = 10}) async {
    if (query.trim().length < 2) return [];

    final results = <SearchResult>[];
    final seenCoords = <String>{};

    // 1. Backend DB araması
    try {
      final token = await _auth.getAccessToken();
      if (token != null) {
        final url = Uri.parse(
          '${AppConfig.apiEndpoint}/locations/search?q=${Uri.encodeComponent(query.trim())}&limit=$limit',
        );
        final response = await http.get(
          url,
          headers: {
            'Content-Type': 'application/json',
            'Authorization': 'Bearer $token',
          },
        );

        _logHttp(
          method: 'GET',
          url: '/locations/search?q=${query.trim()}',
          statusCode: response.statusCode,
        );

        if (response.statusCode == 200) {
          final data = jsonDecode(response.body) as Map<String, dynamic>;
          final items = data['results'] as List<dynamic>;
          for (final item in items) {
            final m = item as Map<String, dynamic>;
            final lat = (m['lat'] as num).toDouble();
            final lon = (m['lon'] as num).toDouble();
            final key = '${(lat * 1_000_000).round()}_${(lon * 1_000_000).round()}';
            if (seenCoords.contains(key)) continue;
            seenCoords.add(key);
            results.add(SearchResult(
              displayName: (m['name'] as String?) ?? (m['address'] as String?) ?? '',
              coordinate: LatLng(lat, lon),
              source: m['source'] as String? ?? 'db',
              id: (m['id'] as num?)?.toInt(),
            ));
          }
        }
      }
    } catch (e) {
      debugPrint('LocationSearchApi.search (DB) error: $e');
    }

    // 2. DB'den yeterli sonuç gelmezse ORS fallback
    if (results.length < limit) {
      try {
        final orsResults = await RouteService.searchAddress(query);
        final remaining = limit - results.length;
        for (final r in orsResults.take(remaining)) {
          final key =
              '${(r.coordinate.latitude * 1_000_000).round()}_${(r.coordinate.longitude * 1_000_000).round()}';
          if (seenCoords.contains(key)) continue;
          seenCoords.add(key);
          results.add(SearchResult(
            displayName: r.displayName,
            coordinate: r.coordinate,
            source: 'nominatim',
          ));
        }
      } catch (e) {
        debugPrint('LocationSearchApi.search (ORS fallback) error: $e');
      }
    }

    return results;
  }
}