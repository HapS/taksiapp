import 'dart:async';
import 'dart:math';
import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';
import '../services/route_service.dart';
import '../services/ride_service.dart';
import '../services/ride_ws_service.dart';

/// Uygulamanın genel ride state'ini tutan veri sınıfı.
///
/// Riverpod_Notifier ile yönetilir; UI bu state'e göre kendini çizer.
///
/// Backend ilişkisi:
/// - currentLocation: GPS'ten alınır, requestRide() → POST /api/ride/request body'sine pickup_lat/pickup_lon olarak yazılır
/// - destination/destinationAddress: ORS autocomplete'ten alınır, requestRide() body'sine dropoff_lat/dropoff_lon/dropoff_address olarak yazılır
/// - activeRideId: POST /api/ride/request yanıtından gelir (ride_id)
/// - rideStatus: WS mesajları (ride_status_changed) + polling (GET /api/ride/:id status alanı) ile güncellenir
/// - assignedDriver: polling'den (GET /api/ride/:id → driver objesi) alınır
/// - driverLocation: WS mesajları (driver_location) + polling'den (driver_lat/driver_lon) alınır
/// - fareAmount: polling'den (GET /api/ride/:id → fare_amount) alınır
/// - etaSeconds: polling'den (GET /api/ride/:id → duration_sec) alınır, routeInfo yoksa ETA hesaplamasında kullanılır
/// - routePoints/routeInfo: ORS API'den hesaplanır, backend'e gönderilmez
class RideState {
  final LatLng? currentLocation;
  final LatLng? destination;
  final String? destinationAddress;
  final List<LatLng> routePoints;
  final RouteInfo? routeInfo;

  final int? activeRideId;
  final String rideStatus; // idle, searching, accepted, picked_up, completed, cancelled
  final DriverInfo? assignedDriver;
  final LatLng? driverLocation;
  final double? fareAmount;
  final int? etaSeconds;
  /// Sürücünün yolcuya ulaşma süresi (saniye). driverLocation + pickup noktası
  /// arasında ORS rota hesaplaması ile güncellenir.
  final int? driverEtaSeconds;
  /// Sürücünün hareket yönü (derece). 0=Kuzey, 90=Doğu, 180=Güney, 270=Batı.
  /// Ardışık konum güncellemeleri arası bearing ile hesaplanır.
  final double? driverHeading;
  /// Backend'den gelen ücret detayı — açılış ücreti, min ücret, km başına ücret, tahmini tutar.
  /// routeInfo.fareInfo ile aynı veridir; ride aktif olduğunda polling'den de güncellenir.
  final FareInfo? fareInfo;
  /// Görünen harita alanı içindeki müsait sürücüler.
  final List<NearbyDriver> nearbyDrivers;

  RideState({
    this.currentLocation,
    this.destination,
    this.destinationAddress,
    this.routePoints = const [],
    this.routeInfo,
    this.activeRideId,
    this.rideStatus = 'idle',
    this.assignedDriver,
    this.driverLocation,
    this.fareAmount,
    this.etaSeconds,
    this.driverEtaSeconds,
    this.driverHeading,
    this.fareInfo,
    this.nearbyDrivers = const [],
  });

  RideState copyWith({
    LatLng? currentLocation,
    LatLng? destination,
    String? destinationAddress,
    List<LatLng>? routePoints,
    RouteInfo? routeInfo,
    Object? activeRideId = _sentinel,
    String? rideStatus,
    Object? assignedDriver = _sentinel,
    Object? driverLocation = _sentinel,
    Object? fareAmount = _sentinel,
    Object? etaSeconds = _sentinel,
    Object? driverEtaSeconds = _sentinel,
    Object? driverHeading = _sentinel,
    Object? fareInfo = _sentinel,
    Object? nearbyDrivers = _sentinel,
  }) {
    return RideState(
      currentLocation: currentLocation ?? this.currentLocation,
      destination: destination ?? this.destination,
      destinationAddress: destinationAddress ?? this.destinationAddress,
      routePoints: routePoints ?? this.routePoints,
      routeInfo: routeInfo ?? this.routeInfo,
      activeRideId: activeRideId == _sentinel ? this.activeRideId : activeRideId as int?,
      rideStatus: rideStatus ?? this.rideStatus,
      assignedDriver: assignedDriver == _sentinel ? this.assignedDriver : assignedDriver as DriverInfo?,
      driverLocation: driverLocation == _sentinel ? this.driverLocation : driverLocation as LatLng?,
      fareAmount: fareAmount == _sentinel ? this.fareAmount : fareAmount as double?,
      etaSeconds: etaSeconds == _sentinel ? this.etaSeconds : etaSeconds as int?,
      driverEtaSeconds: driverEtaSeconds == _sentinel ? this.driverEtaSeconds : driverEtaSeconds as int?,
      driverHeading: driverHeading == _sentinel ? this.driverHeading : driverHeading as double?,
      fareInfo: fareInfo == _sentinel ? this.fareInfo : fareInfo as FareInfo?,
      nearbyDrivers: nearbyDrivers == _sentinel ? this.nearbyDrivers : nearbyDrivers as List<NearbyDriver>,
    );
  }

  RideState clearRoute() {
    return RideState(
      currentLocation: currentLocation,
      destination: null,
      destinationAddress: null,
      routePoints: [],
      routeInfo: null,
      activeRideId: activeRideId,
      rideStatus: rideStatus,
      assignedDriver: assignedDriver,
      driverLocation: driverLocation,
      fareAmount: fareAmount,
      fareInfo: null,
      nearbyDrivers: nearbyDrivers,
    );
  }

  /// Varsayılan lokasyon - Sakarya Serdivan
  static const LatLng defaultLocation = LatLng(40.7604062, 30.3629614);
}

const _sentinel = Object();

/// Riverpod Notifier — uygulama genelinde ride state'ini yönetir.
///
/// WS bağlantısı, polling, rota/destination/state güncellemeleri burada yönetilir.
///
/// Backend ilişkisi:
/// - requestRide()  → POST /api/ride/request
/// - cancelRide()   → POST /api/ride/:id/cancel
/// - completeRide() → POST /api/ride/:id/status {status: completed}
/// - _connectWs()   → WS /ws/passenger?user_id=X (server push: driver_location, ride_status_changed, offer_expired)
/// - _startPolling() → GET /api/ride/:id (5sn aralıkla: status, driver, driver_lat/lon, fare_amount, duration_sec)
class RideNotifier extends Notifier<RideState> {
  final _wsService = RideWsService();
  StreamSubscription<dynamic>? _wsSub;
  Timer? _pollTimer;
  LatLng? _prevDriverLocation;

  @override
  RideState build() => RideState();

  /// Kullanıcının GPS konumunu state'e yazar.
  /// Backend'e doğrudan gönderilmez; requestRide() çağrısında pickup_lat/pickup_lon olarak kullanılır.
  void setCurrentLocation(LatLng location) {
    state = state.copyWith(currentLocation: location);
  }

  /// Görünen harita alanı içindeki müsait sürücüleri çeker ve state'e yazar.
  /// Harita viewport değiştiğinde çağrılır.
  Future<void> fetchNearbyDrivers({
    required double minLat,
    required double maxLat,
    required double minLon,
    required double maxLon,
    String status = 'available',
  }) async {
    final drivers = await RideService.getNearbyDrivers(
      minLat: minLat,
      maxLat: maxLat,
      minLon: minLon,
      maxLon: maxLon,
      status: status,
    );
    if (drivers.isNotEmpty || state.nearbyDrivers.isNotEmpty) {
      state = state.copyWith(nearbyDrivers: drivers);
    }
  }

  /// Hedef noktasını ve adresini state'e yazar.
  /// Backend'e requestRide() çağrısında dropoff_lat/dropoff_lon/dropoff_address olarak gönderilir.
  void setDestination(LatLng destination, [String? address]) {
    state = state.copyWith(
      destination: destination,
      destinationAddress: address,
    );
  }

  /// ORS API'den alınan rota noktalarını ve süre/mesafe bilgisini state'e yazar.
  /// fareInfo backend'den geliyorsa onu da state'e ekler.
  void setRoute(List<LatLng> points, RouteInfo info) {
    state = state.copyWith(
      routePoints: points,
      routeInfo: info,
      fareInfo: info.fareInfo,
    );
  }

  /// Rotayı ve hedefi temizler. Sadece idle durumunda çalışır.
  /// Arama kutusundaki X butonuna basıldığında çağrılır.
  void clearRoute() {
    if (state.rideStatus != 'idle') return;
    state = RideState(currentLocation: state.currentLocation);
  }

  /// Yeni bir yolculuk talebi oluşturur.
  ///
  /// 1. State'i 'searching'e çeker
  /// 2. WS bağlantısını açar (_connectWs)
  /// 3. POST /api/ride/request ile backend'e request gönderir
  /// 4. Başarılı yanıttan ride_id'yi state'e yazar
  /// 5. Polling başlatır (_startPolling)
  ///
  /// Backend: POST /api/ride/request
  /// Body: { user_id, pickup_lat, pickup_lon, pickup_address, dropoff_lat, dropoff_lon, dropoff_address }
  /// Yanıt: { ride_id, status }
  Future<void> requestRide() async {
    final current = state.currentLocation;
    final dest = state.destination;
    final destAddress = state.destinationAddress ?? '';

    if (current == null || dest == null) {
      debugPrint('RideNotifier: konum veya hedef eksik');
      return;
    }

    state = state.copyWith(rideStatus: 'searching');

    // WS bağlantısını ride isteğinden ÖNCE aç
    await _connectWs();

    final response = await RideService.requestRide(
      pickupLat: current.latitude,
      pickupLon: current.longitude,
      pickupAddress: 'Mevcut Konum',
      dropoffLat: dest.latitude,
      dropoffLon: dest.longitude,
      dropoffAddress: destAddress,
    );

    if (!response.success || response.rideId == null) {
      debugPrint('RideNotifier: requestRide failed: ${response.error}');
      _wsSub?.cancel();
      _wsService.disconnect();
      state = state.copyWith(rideStatus: 'idle');
      return;
    }

    final rideId = response.rideId!;
    debugPrint('RideNotifier: ride created id=$rideId');
    state = state.copyWith(
      activeRideId: rideId,
      rideStatus: response.status ?? 'searching',
    );

    // Polling başlat
    _startPolling(rideId);
  }

/// WebSocket bağlantısını açar ve mesaj dinleyicisini kaydeder.
  ///
  /// JWT token ile WS endpoint'ine bağlanır (token WS URL'ine query param olarak eklenir).
  /// Serverdan gelecek mesajlar: driver_location, ride_status_changed, offer_expired, pong, error
  ///
  /// Backend: WS /ws/passenger?token=`<jwt>` (handler.rs verify_ws_token ile doğrular)
  Future<void> _connectWs() async {
    await _wsService.connect();

    _wsSub = _wsService.messages.listen(
      _handleWsMessage,
      onError: (e) => debugPrint('RideNotifier: WS stream error: $e'),
    );
  }

  /// WS'den gelen server mesajlarını işler.
  void _handleWsMessage(ServerMessage msg) {
    switch (msg) {
      case RideStatusChangedMessage():
        debugPrint('══════ YOLCU WS: Durum Değişikliği ══════');
        debugPrint('ride_id=${msg.rideId} status=${msg.status}');
        debugPrint('═══════════════════════════════════════');
        if (msg.status == 'no_driver') {
          _handleNoDriver();
        } else {
          state = state.copyWith(rideStatus: msg.status);
          if (msg.status == 'completed' || msg.status == 'cancelled') {
            _stopPolling();
          }
        }
      case DriverLocationMessage():
        debugPrint('══════ YOLCU WS: Sürücü Konumu ══════');
        debugPrint('ride_id=${msg.rideId} lat=${msg.lat} lon=${msg.lon}');
        debugPrint('══════════════════════════════════');
        _updateDriverPosition(LatLng(msg.lat, msg.lon));
        _updateDriverEta(LatLng(msg.lat, msg.lon));
      case OfferExpiredMessage():
        debugPrint('══════ YOLCU WS: Teklif Süresi Doldu ══════');
        debugPrint('ride_id=${msg.rideId}');
        debugPrint('══════════════════════════════════════');
        _handleNoDriver();
      case RideOfferMessage():
        debugPrint('══════ YOLCU WS: RideOffer ══════');
        debugPrint('${msg.data}');
        debugPrint('════════════════════════════');
        break;
      case PongMessage():
        debugPrint('RideNotifier: pong received');
      case ErrorMessage():
        debugPrint('RideNotifier: WS error: ${msg.message}');
    }
  }

  /// 5 saniyede bir GET /api/ride/:id çağrısı yaparak ride durumunu ve sürücü konumunu günceller.
  ///
  /// WS kesintilerinde fallback olarak çalışır. WS mesajı gelse bile polling devam eder
  /// çünkü polling driver bilgisi (isim, telefon, plaka) ve fare_amount gibi
  /// WS'de gelmeyen ek alanları da taşır.
  ///
  /// Backend: GET /api/ride/:id
  /// Yanıt: { id, status, driver{...}, distance_km, duration_sec, fare_amount, driver_lat, driver_lon }
  void _startPolling(int rideId) {
    _pollTimer?.cancel();
    _pollTimer = Timer.periodic(const Duration(seconds: 5), (_) async {
      final response = await RideService.getRideStatus(rideId);
      if (response.success && response.status != null) {
        debugPrint('RideNotifier: poll status=${response.status}');
        if (response.driverLat != null && response.driverLon != null) {
          final driverPos = LatLng(response.driverLat!, response.driverLon!);
          _updateDriverPosition(driverPos);
          // _updateDriverPosition already set driverLocation + driverHeading
          // so just update the remaining fields
          state = state.copyWith(
            rideStatus: response.status,
            assignedDriver: response.driver,
            fareAmount: response.fareAmount,
            fareInfo: response.fareInfo,
            etaSeconds: response.durationSec,
          );
          _updateDriverEta(driverPos);
        } else {
          state = state.copyWith(
            rideStatus: response.status,
            assignedDriver: response.driver,
            fareAmount: response.fareAmount,
            fareInfo: response.fareInfo,
            etaSeconds: response.durationSec,
          );
        }
        if (response.status == 'no_driver') {
          _handleNoDriver();
        } else if (response.status == 'completed') {
          _stopPolling();
        }
      }
    });
  }

  /// Polling timer'ını durdurur.
  void _stopPolling() {
    _pollTimer?.cancel();
    _pollTimer = null;
  }

  /// Sürücü konumu güncellendiğinde, sürücünün yolcuya ulaşma süresini hesaplar.
  /// Sürücü konumu → pickup noktası arasında ORS rota çeker.
  Future<void> _updateDriverEta(LatLng driverPos) async {
    if (state.rideStatus != 'accepted') return;
    final pickup = state.currentLocation;
    if (pickup == null) return;
    try {
      final route = await RouteService.getRoute(driverPos, pickup);
      if (route != null) {
        state = state.copyWith(driverEtaSeconds: route.durationSeconds);
      }
    } catch (e) {
      debugPrint('RideNotifier: sürücü ETA hesaplama hatası: $e');
    }
  }

  /// Ardışık sürücü konumlarından bearing (derece) hesaplar.
  /// 0=Kuzey, 90=Doğu, 180=Güney, 270=Batı.
  double _calculateBearing(LatLng from, LatLng to) {
    final lat1 = from.latitude * pi / 180;
    final lat2 = to.latitude * pi / 180;
    final dLon = (to.longitude - from.longitude) * pi / 180;
    final y = sin(dLon) * cos(lat2);
    final x = cos(lat1) * sin(lat2) - sin(lat1) * cos(lat2) * cos(dLon);
    final bearing = atan2(y, x) * 180 / pi;
    return (bearing + 360) % 360;
  }

  /// Sürücü konumunu ve heading'i günceller.
  void _updateDriverPosition(LatLng newPos) {
    double? heading;
    if (_prevDriverLocation != null) {
      final dist = _haversineMeters(_prevDriverLocation!, newPos);
      if (dist > 2.0) {
        heading = _calculateBearing(_prevDriverLocation!, newPos);
        _prevDriverLocation = newPos;
      }
    } else {
      _prevDriverLocation = newPos;
    }
    state = state.copyWith(
      driverLocation: newPos,
      driverHeading: heading,
    );
  }

  /// no_driver durumunda otomatik tekrar dener (kullanıcı iptal edene kadar).
  void _handleNoDriver() {
    debugPrint('RideNotifier: no_driver, 2sn sonra tekrar deneniyor...');
    _stopPolling();
    _wsSub?.cancel();
    _wsService.disconnect();
    state = state.copyWith(rideStatus: 'searching');
    Future.delayed(const Duration(seconds: 2), () => requestRide());
  }

  /// Yolculuğu iptal eder.
  ///
  /// Backend: POST /api/ride/:id/cancel
  /// Body: { by: "passenger" }
  ///
  /// Sonra WS bağlantısını keser ve state'i sıfırlar.
  Future<void> cancelRide() async {
    final rideId = state.activeRideId;
    if (rideId != null) {
      await RideService.cancelRide(rideId, by: 'passenger');
    }
    _stopPolling();
    _wsSub?.cancel();
    _wsService.disconnect();
    _prevDriverLocation = null;
    state = RideState(currentLocation: state.currentLocation);
  }

  /// Yolculuğu tamamlandı olarak işaretler (yolcu tarafı).
  ///
  /// Backend: POST /api/ride/:id/status
  /// Body: { status: "completed" }
  ///
  /// Backend accepted/picked_up → completed geçişini yapar,
  /// WS üzerinden ride_status_changed mesajı yayınlar.
  Future<void> completeRide() async {
    final rideId = state.activeRideId;
    if (rideId != null) {
      await RideService.updateRideStatus(rideId, 'completed');
    }
  }

  /// Verilen rideId ile completed/cancelled işlemi yapar.
  /// Yarım kalan ride dialog'undan çağrılır.
  Future<void> completeRideById(int rideId) async {
    await RideService.updateRideStatus(rideId, 'completed');
  }

  /// Yarım kalan ride'ı provider'a restore eder.
  /// Aktif ride state'ini kurar, polling ve WS'yi yeniden başlatır.
  void restoreActiveRide({
    required int rideId,
    required String status,
    required String pickupAddress,
    required String dropoffAddress,
    required double pickupLat,
    required double pickupLon,
    required double dropoffLat,
    required double dropoffLon,
    double? distanceKm,
    int? durationSec,
  }) {
    debugPrint('RideNotifier: restoreActiveRide rideId=$rideId status=$status');

    state = state.copyWith(
      activeRideId: rideId,
      rideStatus: status,
      destination: LatLng(dropoffLat, dropoffLon),
      destinationAddress: dropoffAddress,
    );

    _connectWs();
    _startPolling(rideId);
  }

  /// Ride state'ini sıfırlar (polling + WS kes + state temizle).
  ///
  /// "Kapat" butonları ve "Tekrar Dene" akışında kullanılır.
  /// cancelRide()'tan farklı olarak backend'e istek göndermez;
  /// sadece client tarafını temizler.
  ///
  /// [keepDestination] = true → destination, destinationAddress, routePoints, routeInfo korunur.
  /// "Tekrar Dene" butonunda kullanılır; kullanıcı hedefini yeniden girmek zorunda kalmaz.
  void resetRide({bool keepDestination = false}) {
    _stopPolling();
    _wsSub?.cancel();
    _wsService.disconnect();
    _prevDriverLocation = null;
    if (keepDestination) {
      state = RideState(
        currentLocation: state.currentLocation,
        destination: state.destination,
        destinationAddress: state.destinationAddress,
        routePoints: state.routePoints,
        routeInfo: state.routeInfo,
        fareInfo: state.fareInfo,
        nearbyDrivers: state.nearbyDrivers,
      );
    } else {
      state = RideState(
        currentLocation: state.currentLocation,
        nearbyDrivers: state.nearbyDrivers,
      );
    }
  }

  /// Resource'ları serbest bırakır. Provider dispose olduğunda çağrılır.
  void dispose() {
    _stopPolling();
    _wsSub?.cancel();
    _wsService.disconnect();
  }
}

/// Ride state provider — tüm uygulama genelinde erişilebilir.
///
/// Kullanım: ref.watch(rideProvider) → state oku
///           ref.read(rideProvider.notifier) → metod çağır
final rideProvider = NotifierProvider<RideNotifier, RideState>(() {
  return RideNotifier();
});

/// İki nokta arası Haversine mesafesi (metre).
double _haversineMeters(LatLng a, LatLng b) {
  const r = 6371000.0;
  final dLat = (b.latitude - a.latitude) * pi / 180;
  final dLon = (b.longitude - a.longitude) * pi / 180;
  final lat1 = a.latitude * pi / 180;
  final lat2 = b.latitude * pi / 180;
  final x = sin(dLat / 2) * sin(dLat / 2) +
      cos(lat1) * cos(lat2) * sin(dLon / 2) * sin(dLon / 2);
  return r * 2 * atan2(sqrt(x), sqrt(1 - x));
}