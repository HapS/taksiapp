import 'dart:async';
import 'dart:convert';
import 'dart:math';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:latlong2/latlong.dart';
import 'package:geolocator/geolocator.dart';
import 'package:go_router/go_router.dart';
import 'package:web_socket_channel/web_socket_channel.dart';
import 'package:http/http.dart' as http;
import 'package:awesome_dialog/awesome_dialog.dart';
import '../auth/services/auth_service.dart';
import '../../../core/config/app_config.dart';
import '../auth/providers/auth_provider.dart';
import 'services/route_service.dart' show FareInfo, RouteService;
import 'services/ride_service.dart';

/// Sürücü ana ekranı.
///
/// Harita tabanlı UI; sürücü online/offline olabilir,
/// gelen ride tekliflerini kabul/reddedebilir,
/// konumunu sürekli backend'e gönderir.
///
/// Backend ilişkisi:
/// - WS /ws/driver?token=`<jwt>` — JWT ile kimlik doğrulama
/// - location_update mesajları — her 3 saniyede sürücü konumu backend'e gönderilir
/// - ride_offer mesajı — backend'den gelen yolculuk teklifi
/// - offer_response mesajı — teklif kabul/reddetme yanıtı backend'e gönderilir
/// - ride_status_changed mesajı — yolculuk durumu değişikliği bildirimi
class DriverHomePage extends ConsumerStatefulWidget {
  const DriverHomePage({super.key});

  @override
  ConsumerState<DriverHomePage> createState() => _DriverHomePageState();
}

class _DriverHomePageState extends ConsumerState<DriverHomePage> {
  final MapController _mapController = MapController();
  final GlobalKey<ScaffoldState> _scaffoldKey = GlobalKey<ScaffoldState>();

  bool _isOnline = false;
  LatLng? _currentLocation;
  double? _driverHeading;
  LatLng? _prevLocation;
  double? _pickupRouteKm;
  int? _pickupRouteMin;

  WebSocketChannel? _wsChannel;
  Timer? _pingTimer;
  Timer? _locationTimer;

  Map<String, dynamic>? _pendingOffer;
  Timer? _offerTimer;
  int _offerCountdown = 30;

  bool _hasActiveRide = false;
  Map<String, dynamic>? _activeRideInfo;
  List<LatLng> _routeToPickup = [];
  List<LatLng> _routeToDropoff = [];
  String _ridePhase = 'idle'; // idle, offered, driving_to_pickup, picked_up
  bool _isEndingRide = false;
  /// Geçici: ileride backend driver profilinden gelecek.
  /// true → sürücü teklif anında rotayı haritada görür.
  bool _userMapEnabled = true;

  static const LatLng _defaultCenter = LatLng(40.7604062, 30.3629614);

  @override
  void initState() {
    super.initState();
    _initLocation();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      _checkAndRestoreActiveRide();
    });
  }

  @override
  void dispose() {
    _disconnect();
    _locationTimer?.cancel();
    _offerTimer?.cancel();
    super.dispose();
  }

  /// GPS'ten sürücünün mevcut konumunu alır ve haritayı ortalar.
  Future<void> _initLocation() async {
    try {
      LocationPermission perm = await Geolocator.checkPermission();
      if (perm == LocationPermission.denied) {
        perm = await Geolocator.requestPermission();
      }
      if (perm == LocationPermission.deniedForever) return;

      final pos = await Geolocator.getCurrentPosition(
        locationSettings: const LocationSettings(
          accuracy: LocationAccuracy.high,
          timeLimit: Duration(seconds: 5),
        ),
      );
      final loc = LatLng(pos.latitude, pos.longitude);
      _updateLocation(loc, pos.heading);
      _mapController.move(loc, 15);
    } catch (e) {
      debugPrint('DriverHomePage: konum alınamadı: $e');
    }
  }

  Future<void> _checkAndRestoreActiveRide() async {
    final activeRide = await RideService.getDriverActiveRide();
    if (activeRide == null) return;
    if (!mounted) return;

    debugPrint('DriverHomePage: Yarım kalan ride bulundu: #${activeRide.rideId} (${activeRide.status})');

    await Future.delayed(const Duration(milliseconds: 500));
    if (!mounted) return;

    final result = await showDialog<String>(
      context: context,
      barrierDismissible: false,
      builder: (ctx) => AlertDialog(
        title: const Text('Yarım Kalan Yolculuk'),
        content: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text('${activeRide.pickupAddress} → ${activeRide.dropoffAddress}'),
            const SizedBox(height: 16),
            const Text('Bu yolculuğa nasıl devam etmek istersiniz?'),
          ],
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(ctx).pop('cancel'),
            style: TextButton.styleFrom(foregroundColor: Colors.red),
            child: const Text('İptal'),
          ),
          TextButton(
            onPressed: () => Navigator.of(ctx).pop('continue'),
            style: TextButton.styleFrom(foregroundColor: Colors.blue),
            child: const Text('Devam'),
          ),
          FilledButton(
            onPressed: () => Navigator.of(ctx).pop('completed'),
            style: FilledButton.styleFrom(backgroundColor: Colors.green),
            child: const Text('Tamamlandı'),
          ),
        ],
      ),
    );

    if (!mounted) return;
    switch (result) {
      case 'cancel':
        await _resolveStaleRide(activeRide.rideId, 'cancelled');
      case 'continue':
        await _continueRide(activeRide);
      case 'completed':
        await _resolveStaleRide(activeRide.rideId, 'completed');
    }
  }

  /// GPS konum ve heading günceller. heading NaN veya 0 ise
  /// ardışık konumlardan bearing hesaplar.
  void _updateLocation(LatLng loc, double gpsHeading) {
    double? heading;
    if (!gpsHeading.isNaN && gpsHeading > 0) {
      heading = gpsHeading;
    } else if (_prevLocation != null) {
      final dist = _haversineMeters(_prevLocation!, loc);
      if (dist > 2.0) {
        heading = _calculateBearing(_prevLocation!, loc);
      }
    }
    if (_prevLocation == null || (_prevLocation! != loc && _haversineMeters(_prevLocation!, loc) > 2.0)) {
      _prevLocation = loc;
    }
    setState(() {
      _currentLocation = loc;
      if (heading != null) _driverHeading = heading;
    });
  }

  double _calculateBearing(LatLng from, LatLng to) {
    final lat1 = from.latitude * pi / 180;
    final lat2 = to.latitude * pi / 180;
    final dLon = (to.longitude - from.longitude) * pi / 180;
    final y = sin(dLon) * cos(lat2);
    final x = cos(lat1) * sin(lat2) - sin(lat1) * cos(lat2) * cos(dLon);
    return (atan2(y, x) * 180 / pi + 360) % 360;
  }

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

  /// Yarım kalan ride'ı olduğu gibi devam ettirir.
  /// accepted → pickup'a yönlendir, picked_up → dropoff'a yönlendir.
  Future<void> _continueRide(ActiveRideInfo activeRide) async {
    debugPrint('DriverHomePage: Ride #${activeRide.rideId} devam ediyor (${activeRide.status})');

    final isPickedUp = activeRide.status == 'picked_up';

    setState(() {
      _hasActiveRide = true;
      _activeRideInfo = {
        'ride_id': activeRide.rideId,
        'status': activeRide.status,
        'pickup_address': activeRide.pickupAddress,
        'dropoff_address': activeRide.dropoffAddress,
        'pickup_lat': activeRide.pickupLat,
        'pickup_lon': activeRide.pickupLon,
        'dropoff_lat': activeRide.dropoffLat,
        'dropoff_lon': activeRide.dropoffLon,
        'distance_km': activeRide.distanceKm,
        'duration_sec': activeRide.durationSec,
        'fare_amount': activeRide.fareAmount,
        'fare_info': activeRide.fareInfo?.toJson(),
      };
      _ridePhase = isPickedUp ? 'picked_up' : 'driving_to_pickup';
      _pendingOffer = null;
    });

    if (isPickedUp) {
      // picked_up: sadece pickup→dropoff rotası
      _routeToPickup = [];
      _pickupRouteKm = null;
      _pickupRouteMin = null;
      _pickupRouteKm = null;
      _pickupRouteMin = null;
      await _fetchRouteToDropoff(
        activeRide.pickupLat, activeRide.pickupLon,
        activeRide.dropoffLat, activeRide.dropoffLon,
      );
      _mapController.move(LatLng(activeRide.dropoffLat, activeRide.dropoffLon), 14);
      if (mounted) {
        _showAutoDialog(
          dialogType: DialogType.success,
          title: 'Yolculuk Devam Ediyor',
          desc: 'Varış noktasına ilerleyin.',
        );
      }
    } else {
      // accepted: sürücü konumundan pickup'a, sonra pickup→dropoff
      await _fetchRouteToPickup(activeRide.pickupLat, activeRide.pickupLon);
      await _fetchRouteToDropoff(
        activeRide.pickupLat, activeRide.pickupLon,
        activeRide.dropoffLat, activeRide.dropoffLon,
      );
      _mapController.move(LatLng(activeRide.pickupLat, activeRide.pickupLon), 14);
      if (mounted) {
        _showAutoDialog(
          dialogType: DialogType.info,
          title: 'Yolculuk Devam Ediyor',
          desc: 'Alma noktasına ilerleyin.',
        );
      }
    }
  }

  /// Yarım kalan ride'ı çözümler ve UI'ı temizler.
  Future<void> _resolveStaleRide(int rideId, String resolution) async {
    debugPrint('DriverHomePage: Stale ride #$rideId → $resolution');

    bool ok = false;
    if (resolution == 'completed') {
      ok = await RideService.updateRideStatus(rideId, 'completed');
    } else {
      ok = await RideService.cancelRide(rideId, by: 'driver');
    }

    if (!mounted) return;

    if (ok) {
      debugPrint('DriverHomePage: Stale ride çözümlendi');
      setState(() {
        _hasActiveRide = false;
        _activeRideInfo = null;
        _ridePhase = 'idle';
        _routeToPickup = [];
      _pickupRouteKm = null;
      _pickupRouteMin = null;
        _routeToDropoff = [];
      });
    } else {
      if (!mounted) return;
      showDialog(
        context: context,
        barrierDismissible: false,
        builder: (ctx) => AlertDialog(
          title: const Row(children: [Icon(Icons.error, color: Colors.red), SizedBox(width: 8), Text('Hata')]),
          content: const Text('Yolculuk güncellenemedi. Lütfen internet bağlantınızı kontrol edin.'),
          actions: [TextButton(onPressed: () { Navigator.of(ctx).pop(); _checkAndRestoreActiveRide(); }, child: const Text('Tekrar Dene'))],
        ),
      );
    }
  }

  /// Online/Offline toggle — sürücü durumunu değiştirir.
  ///
  /// Online: WS bağlantısı açılır, konum gönderimi başlar.
  /// Offline: WS bağlantısı kapatılır, konum gönderimi durur.
  Future<void> _toggleOnline() async {
    if (_isOnline) {
      _disconnect();
      setState(() => _isOnline = false);
    } else {
      await _connect();
      if (_wsChannel != null) {
        setState(() => _isOnline = true);
      }
    }
  }

  /// WS bağlantısını JWT token ile açar.
  ///
  /// Backend: /ws/driver?token=`<jwt>`
  /// handler.rs verify_ws_token() ile JWT doğrular,
  /// drivers tablosunda user_id'ye karşılık gelen kaydı bulur.
  Future<void> _connect() async {
    final token = await AuthService().getAccessToken();
    if (token == null) {
      debugPrint('DriverHomePage: token bulunamadı');
      return;
    }

    final uri = Uri.parse(
      '${AppConfig.wsBaseUrl}/ws/driver?token=${Uri.encodeComponent(token)}',
    );
    debugPrint('DriverHomePage: connecting to WS');

    _wsChannel = WebSocketChannel.connect(uri);

    try {
      await _wsChannel!.ready;
      debugPrint('DriverHomePage: WS connected');
    } catch (e) {
      debugPrint('DriverHomePage: WS connection failed: $e');
      _wsChannel = null;
      return;
    }

    _wsChannel!.stream.listen(
      (data) => _handleMessage(data as String),
      onError: (e) {
        debugPrint('DriverHomePage: WS error: $e');
        setState(() => _isOnline = false);
      },
      onDone: () {
        debugPrint('DriverHomePage: WS closed');
        setState(() => _isOnline = false);
      },
    );

    // Ping timer — her 20sn'de bir heartbeat
    _pingTimer = Timer.periodic(const Duration(seconds: 20), (_) {
      _send({'type': 'ping'});
    });

    // Konum gönderme timer — her 3 saniyede backend'e location_update
    _locationTimer = Timer.periodic(const Duration(seconds: 3), (_) async {
      try {
        final pos = await Geolocator.getCurrentPosition();
        final loc = LatLng(pos.latitude, pos.longitude);
        _updateLocation(loc, pos.heading);
        _send({
          'type': 'location_update',
          'lat': loc.latitude,
          'lon': loc.longitude,
        });
      } catch (e) {
        debugPrint('DriverHomePage: konum alınamadı: $e');
      }
    });
  }

  /// WS bağlantısını ve timer'ları kapatır.
  void _disconnect() {
    _pingTimer?.cancel();
    _pingTimer = null;
    _locationTimer?.cancel();
    _locationTimer = null;
    _wsChannel?.sink.close();
    _wsChannel = null;
  }

  /// WS kanalına JSON mesaj gönderir.
  void _send(Map<String, dynamic> data) {
    try {
      final json = jsonEncode(data);
      debugPrint('══════ WS SÜRÜCÜ GÖNDERİLEN ══════');
      debugPrint('$json');
      debugPrint('═══════════════════════════════');
      _wsChannel?.sink.add(json);
    } catch (e) {
      debugPrint('DriverHomePage: send error: $e');
    }
  }

  /// Backend'den gelen WS mesajlarını işler.
  ///
  /// ride_offer: Yeni yolculuk teklifi — popup gösterilir
  /// ride_status_changed: Durum değişikliği — accepted/cancelled/completed bildirimi
  /// pong: Heartbeat yanıtı
  void _handleMessage(String raw) {
    debugPrint('══════ WS SÜRÜCÜ GELEN ══════');
    debugPrint('$raw');
    debugPrint('═══════════════════════════════');
    try {
      final msg = jsonDecode(raw) as Map<String, dynamic>;
      final type = msg['type'] as String?;

      switch (type) {
        case 'ride_offer':
          if (_pendingOffer != null || _hasActiveRide) {
            debugPrint('DriverHomePage: teklif yoksayılıyor — zaten aktif teklif/ride var');
            break;
          }
          _showOfferDialog(msg);
        case 'ride_status_changed':
          final status = msg['status'] as String?;
          debugPrint('DriverHomePage: ride status changed: $status');
          if (status == 'accepted') {
            _startActiveRide(msg);
          } else if (status == 'picked_up') {
            if (!mounted) return;
            setState(() {
              _ridePhase = 'picked_up';
              _routeToPickup = [];
      _pickupRouteKm = null;
      _pickupRouteMin = null;
            });
            final dropoffLat = (_activeRideInfo?['dropoff_lat'] as num?)?.toDouble();
            final dropoffLon = (_activeRideInfo?['dropoff_lon'] as num?)?.toDouble();
            if (dropoffLat != null && dropoffLon != null) {
              _mapController.move(LatLng(dropoffLat, dropoffLon), 14);
            }
            if (mounted) {
              _showAutoDialog(
                dialogType: DialogType.success,
                title: 'Yolcu Alındı!',
                desc: 'Yolculuk başladı, varış noktasına ilerleyin.',
              );
            }
          } else if (status == 'cancelled' || status == 'completed' || status == 'no_driver') {
            _endActiveRide(status);
          } else if (status == 'offer_expired') {
            _offerTimer?.cancel();
            setState(() {
              _pendingOffer = null;
              _routeToPickup = [];
      _pickupRouteKm = null;
      _pickupRouteMin = null;
              _routeToDropoff = [];
              _ridePhase = 'idle';
            });
            if (mounted) {
              _showAutoDialog(
                dialogType: DialogType.warning,
                title: 'Teklif Süresi Doldu',
                desc: 'Bu yolculuk teklifi zaman aşımına uğradı.',
              );
            }
          }
        case 'pong':
          break;
        default:
          debugPrint('DriverHomePage: bilinmeyen mesaj tipi: $type');
      }
    } catch (e) {
      debugPrint('DriverHomePage: parse error: $e');
    }
  }

  /// Teklif popup'ını gösterir ve 30 saniye geri sayım başlatır.
  void _showOfferDialog(Map<String, dynamic> offer) {
    setState(() {
      _pendingOffer = offer;
      _offerCountdown = 30;
      _ridePhase = 'offered';
    });

    // Premium: teklif anında rotayı haritada göster
    if (_userMapEnabled) {
      final pickupLat = (offer['pickup_lat'] as num?)?.toDouble();
      final pickupLon = (offer['pickup_lon'] as num?)?.toDouble();
      final dropoffLat = (offer['dropoff_lat'] as num?)?.toDouble();
      final dropoffLon = (offer['dropoff_lon'] as num?)?.toDouble();

      if (pickupLat != null && pickupLon != null) {
        _fetchRouteToPickup(pickupLat, pickupLon);
        if (dropoffLat != null && dropoffLon != null) {
          _fetchRouteToDropoff(pickupLat, pickupLon, dropoffLat, dropoffLon);
        }
        _mapController.move(LatLng(pickupLat, pickupLon), 14);
      }
    }

    _offerTimer?.cancel();
    _offerTimer = Timer.periodic(const Duration(seconds: 1), (timer) {
      if (_offerCountdown <= 1) {
        timer.cancel();
        setState(() {
          _pendingOffer = null;
          _routeToPickup = [];
      _pickupRouteKm = null;
      _pickupRouteMin = null;
          _routeToDropoff = [];
          _ridePhase = 'idle';
        });
      } else {
        setState(() => _offerCountdown--);
      }
    });
  }

  /// Teklifi kabul eder — backend'e offer_response gönderir.
  ///
  /// _pendingOffer verisi _activeRideInfo'ya aktarılır
  /// çünkü backend'den gelecek accepted mesajı ride detaylarını içermez.
  void _acceptOffer() {
    if (_pendingOffer == null) return;
    final rideId = _pendingOffer!['ride_id'] as int;
    _send({'type': 'offer_response', 'ride_id': rideId, 'accepted': true});
    _offerTimer?.cancel();

    final pickupLat = (_pendingOffer!['pickup_lat'] as num?)?.toDouble();
    final pickupLon = (_pendingOffer!['pickup_lon'] as num?)?.toDouble();
    final dropoffLat = (_pendingOffer!['dropoff_lat'] as num?)?.toDouble();
    final dropoffLon = (_pendingOffer!['dropoff_lon'] as num?)?.toDouble();

    setState(() {
      _hasActiveRide = true;
      _activeRideInfo = Map<String, dynamic>.from(_pendingOffer!);
      _pendingOffer = null;
      _ridePhase = 'driving_to_pickup';
    });

    // Premium'da rotalar zaten çizili, yeniden fetch'e gerek yok
    if (!_userMapEnabled) {
      _routeToPickup = [];
      _pickupRouteKm = null;
      _pickupRouteMin = null;
      _routeToDropoff = [];
      if (pickupLat != null && pickupLon != null) {
        _fetchRouteToPickup(pickupLat, pickupLon);
        if (dropoffLat != null && dropoffLon != null) {
          _fetchRouteToDropoff(pickupLat, pickupLon, dropoffLat, dropoffLon);
        }
      }
    }

    if (pickupLat != null && pickupLon != null) {
      _mapController.move(LatLng(pickupLat, pickupLon), 14);
    }

    debugPrint('DriverHomePage: teklif kabul edildi ride_id=$rideId');
  }

  /// Teklifi reddeder — backend'e offer_response gönderir.
  void _rejectOffer() {
    if (_pendingOffer == null) return;
    final rideId = _pendingOffer!['ride_id'] as int;
    _send({'type': 'offer_response', 'ride_id': rideId, 'accepted': false});
    _offerTimer?.cancel();
    setState(() {
      _pendingOffer = null;
      _routeToPickup = [];
      _pickupRouteKm = null;
      _pickupRouteMin = null;
      _routeToDropoff = [];
      _ridePhase = 'idle';
    });
    debugPrint('DriverHomePage: teklif reddedildi ride_id=$rideId');
  }

  /// Kabul edilen yolculuk bilgisini onaylar.
  ///
  /// Backend'den gelen "accepted" mesajı ile ride aktif hale gelir.
  void _startActiveRide(Map<String, dynamic> msg) {
    _offerTimer?.cancel();
    setState(() {
      _pendingOffer = null;
    });

    if (mounted) {
      _showAutoDialog(
        dialogType: DialogType.success,
        title: 'Yolculuk Onaylandı!',
        desc: 'Yolcuya doğru ilerleyin.',
      );
    }
  }

  /// 2 saniye sonra otomatik kapanan dialog gösterir.
  void _showAutoDialog({
    required DialogType dialogType,
    required String title,
    required String desc,
  }) {
    if (!mounted) return;
    final color = switch (dialogType) {
      DialogType.success => Colors.green,
      DialogType.warning => Colors.orange,
      DialogType.error => Colors.red,
      DialogType.info => Colors.blue,
      _ => Colors.blue,
    };
    final icon = switch (dialogType) {
      DialogType.success => Icons.check_circle,
      DialogType.warning => Icons.warning_amber,
      DialogType.error => Icons.error,
      DialogType.info => Icons.info,
      _ => Icons.info,
    };
    showDialog(
      context: context,
      barrierDismissible: true,
      builder: (ctx) => AlertDialog(
        title: Row(children: [Icon(icon, color: color), const SizedBox(width: 8), Text(title)]),
        content: Text(desc),
        actions: [TextButton(onPressed: () => Navigator.of(ctx).pop(), child: const Text('Tamam'))],
      ),
    );
  }

  /// Aktif yolculuğu sonlandırır.
  void _endActiveRide(String? status) {
    if (_isEndingRide) return; // WS + HTTP çift çağrı koruması
    _isEndingRide = true;

    _offerTimer?.cancel();
    setState(() {
      _hasActiveRide = false;
      _activeRideInfo = null;
      _routeToPickup = [];
      _pickupRouteKm = null;
      _pickupRouteMin = null;
      _routeToDropoff = [];
      _pendingOffer = null;
      _ridePhase = 'idle';
    });

    if (!mounted) return;

    if (status == 'cancelled') {
      _showAutoDialog(
        dialogType: DialogType.warning,
        title: 'Yolcu İptal Etti',
        desc: 'Yolcu yolculuğu iptal etti.',
      );
    } else if (status == 'completed') {
      _showAutoDialog(
        dialogType: DialogType.success,
        title: 'Yolculuk Tamamlandı!',
        desc: 'Harika iş! Yeni teklif bekleniyor.',
      );
    }

    Future.delayed(const Duration(seconds: 3), () {
      _isEndingRide = false;
    });
  }

  /// Yolcuyu aldım — backend'e picked_up status'ünü gönderir.
  Future<void> _pickUpPassenger() async {
    if (_activeRideInfo == null) return;
    final rideId = _activeRideInfo!['ride_id'] as int;

    try {
      final token = await AuthService().getAccessToken();
      if (token == null) return;

      final response = await http.post(
        Uri.parse('${AppConfig.apiEndpoint}/ride/$rideId/status'),
        headers: {
          'Content-Type': 'application/json',
          'Authorization': 'Bearer $token',
        },
        body: jsonEncode({'status': 'picked_up'}),
      );

      if (response.statusCode == 200) {
        setState(() {
          _ridePhase = 'picked_up';
          _routeToPickup = [];
          _pickupRouteKm = null;
          _pickupRouteMin = null;
        });
        final dropoffLat = (_activeRideInfo?['dropoff_lat'] as num?)?.toDouble();
        final dropoffLon = (_activeRideInfo?['dropoff_lon'] as num?)?.toDouble();
        if (dropoffLat != null && dropoffLon != null) {
          _fetchRouteToDropoff(
            (_activeRideInfo!['pickup_lat'] as num).toDouble(),
            (_activeRideInfo!['pickup_lon'] as num).toDouble(),
            dropoffLat, dropoffLon,
          );
          _mapController.move(LatLng(dropoffLat, dropoffLon), 14);
        }
      } else {
        if (mounted) {
          _showAutoDialog(
            dialogType: DialogType.error,
            title: 'Hata',
            desc: 'Yolcu alınılamadı, tekrar deneyin.',
          );
        }
      }
    } catch (e) {
      debugPrint('DriverHomePage: pickUpPassenger hatası: $e');
      if (mounted) {
        _showAutoDialog(
          dialogType: DialogType.error,
          title: 'Hata',
          desc: 'Bağlantı hatası, tekrar deneyin.',
        );
      }
    }
  }

  /// Yolculuğu tamamlandı olarak işaretler — backend'e HTTP POST gönderir.
  Future<void> _completeRide() async {
    if (_activeRideInfo == null) return;
    final rideId = _activeRideInfo!['ride_id'] as int;

    try {
      final token = await AuthService().getAccessToken();
      if (token == null) return;

      final response = await http.post(
        Uri.parse('${AppConfig.apiEndpoint}/ride/$rideId/status'),
        headers: {
          'Content-Type': 'application/json',
          'Authorization': 'Bearer $token',
        },
        body: jsonEncode({'status': 'completed'}),
      );

      if (response.statusCode == 200) {
        _endActiveRide('completed');
      } else {
        if (mounted) {
          _showAutoDialog(
            dialogType: DialogType.error,
            title: 'Hata',
            desc: 'Yolculuk bitirilemedi, tekrar deneyin.',
          );
        }
      }
    } catch (e) {
      debugPrint('DriverHomePage: complete ride error: $e');
      if (mounted) {
        _showAutoDialog(
          dialogType: DialogType.error,
          title: 'Bağlantı Hatası',
          desc: 'Lütfen internet bağlantınızı kontrol edin.',
        );
      }
    }
  }

  /// Sürücü konumundan pickup'a rota çeker (yeşil)
  Future<void> _fetchRouteToPickup(double pickupLat, double pickupLon) async {
    if (_currentLocation == null) return;
    try {
      final route = await RouteService.getRoute(
        LatLng(_currentLocation!.latitude, _currentLocation!.longitude),
        LatLng(pickupLat, pickupLon),
      );
      if (mounted) setState(() {
        _routeToPickup = route.points;
        _pickupRouteKm = route.distanceKm;
        _pickupRouteMin = (route.durationSeconds / 60).round();
      });
    } catch (e) {
      debugPrint('DriverHomePage: pickup rotası çekilemedi: $e');
      final points = await _fetchRoutePoints(
        _currentLocation!.latitude, _currentLocation!.longitude,
        pickupLat, pickupLon,
      );
      if (mounted) setState(() => _routeToPickup = points);
    }
  }

  /// Pickup'tan dropoff'a rota çeker (mavi)
  Future<void> _fetchRouteToDropoff(
    double pickupLat, double pickupLon,
    double dropoffLat, double dropoffLon,
  ) async {
    final points = await _fetchRoutePoints(
      pickupLat, pickupLon,
      dropoffLat, dropoffLon,
    );
    if (mounted) setState(() => _routeToDropoff = points);
  }

  /// Backend'den rota noktalarını çeker
  Future<List<LatLng>> _fetchRoutePoints(
    double startLat, double startLon,
    double endLat, double endLon,
  ) async {
    try {
      final url = Uri.parse(
        '${AppConfig.apiEndpoint}/ride/route'
        '?start_lat=$startLat&start_lon=$startLon'
        '&end_lat=$endLat&end_lon=$endLon',
      );
      final token = await AuthService().getAccessToken();
      final response = await http.Client().get(url, headers: {
        if (token != null) 'Authorization': 'Bearer $token',
      });

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body) as Map<String, dynamic>;
        final points = data['data'] as List?;
        if (points != null && points.length >= 2) {
          return points
              .map((p) => LatLng(
                    (p as Map)['lat'] as double,
                    p['lon'] as double,
                  ))
              .toList();
        }
      } else {
        debugPrint('DriverHomePage: rota HTTP ${response.statusCode}: ${response.body}');
      }
    } catch (e) {
      debugPrint('DriverHomePage: rota çekilemedi: $e');
    }
    // Fallback: düz çizgi
    return [
      LatLng(startLat, startLon),
      LatLng(endLat, endLon),
    ];
  }

  /// Sürücü kartlarında ücret detayını gösterir.
  ///
  /// [info] → _pendingOffer veya _activeRideInfo map'i.
  /// fare_info WS ride_offer mesajından gelir, fare_amount da aynı mesajda.
  Widget _buildDriverFareSection(Map<String, dynamic> info) {
    final fareAmount = (info['fare_amount'] as num?)?.toDouble();
    final fareInfoRaw = info['fare_info'] as Map<String, dynamic>?;
    final fareInfo = fareInfoRaw != null ? FareInfo.fromJson(fareInfoRaw) : null;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        if (fareInfo != null) ...[
          // 3 ücret kalemi
          Row(
            children: [
              _DriverFareChip(
                label: 'Açılış',
                value: fareInfo.formattedOpeningFee,
                color: Colors.blue,
                icon: Icons.flag_outlined,
              ),
              const SizedBox(width: 6),
              _DriverFareChip(
                label: 'Km başına',
                value: fareInfo.formattedPerKm,
                color: Colors.orange,
                icon: Icons.speed_outlined,
              ),
              const SizedBox(width: 6),
              _DriverFareChip(
                label: 'Min. ücret',
                value: fareInfo.formattedMinFare,
                color: Colors.purple,
                icon: Icons.payments_outlined,
              ),
            ],
          ),
          const SizedBox(height: 8),
        ],
        // Tahmini / gerçek tutar
        Container(
          width: double.infinity,
          padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
          decoration: BoxDecoration(
            color: Colors.amber.withAlpha(20),
            borderRadius: BorderRadius.circular(8),
          ),
          child: Row(
            mainAxisAlignment: MainAxisAlignment.spaceBetween,
            children: [
              const Text(
                'Tahmini kazanç',
                style: TextStyle(fontSize: 13, color: Colors.grey),
              ),
              Text(
                fareAmount != null && fareAmount > 0
                    ? '₺${fareAmount.toStringAsFixed(2)}'
                    : fareInfo?.formattedEstimate ?? '—',
                style: const TextStyle(
                  fontSize: 16,
                  fontWeight: FontWeight.bold,
                  color: Colors.amber,
                ),
              ),
            ],
          ),
        ),
      ],
    );
  }

  /// Saniyeyi "X S Y dk" formatına çevirir (60dk altı sadece "X dk").
  String _formatDuration(int sec) {
    final mins = (sec / 60).ceil();
    if (mins < 60) return '$mins dk';
    final hrs = mins ~/ 60;
    final remainingMins = mins % 60;
    return '$hrs S $remainingMins Dk';
  }

  /// Sürücü ekranı için küçük ücret chip'i
  Widget _DriverFareChip({
    required String label,
    required String value,
    required Color color,
    required IconData icon,
  }) {
    return Expanded(
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 5),
        decoration: BoxDecoration(
          color: color.withAlpha(15),
          borderRadius: BorderRadius.circular(7),
          border: Border.all(color: color.withAlpha(40)),
        ),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                Icon(icon, size: 12, color: color),
                const SizedBox(width: 3),
                Text(label,
                    style: TextStyle(fontSize: 9, color: color, fontWeight: FontWeight.w500)),
              ],
            ),
            const SizedBox(height: 1),
            Text(value,
                style: const TextStyle(fontSize: 12, fontWeight: FontWeight.bold)),
          ],
        ),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final topPadding = MediaQuery.of(context).padding.top;

    return Scaffold(
      key: _scaffoldKey,
      drawer: _buildDrawer(context),
      body: Stack(
        children: [
          FlutterMap(
            mapController: _mapController,
            options: MapOptions(
              initialCenter: _currentLocation ?? _defaultCenter,
              initialZoom: 15,
            ),
            children: [
              TileLayer(
                urlTemplate: 'https://tile.openstreetmap.org/{z}/{x}/{y}.png',
                userAgentPackageName: 'com.example.ride_rs',
              ),
              // Rota hatları — sürücü→pickup (yeşil), pickup→dropoff (mavi)
              if (_routeToPickup.isNotEmpty)
                PolylineLayer(
                  polylines: [
                    Polyline(
                      points: _routeToPickup,
                      color: Colors.green,
                      strokeWidth: 4,
                    ),
                  ],
                ),
              if (_routeToDropoff.isNotEmpty)
                PolylineLayer(
                  polylines: [
                    Polyline(
                      points: _routeToDropoff,
                      color: Colors.blue,
                      strokeWidth: 4,
                    ),
                  ],
                ),
              // Sürücü konum marker'ı
              if (_currentLocation != null)
                MarkerLayer(
                  markers: [
                    Marker(
                      point: _currentLocation!,
                      width: 48,
                      height: 48,
                      child: Transform.rotate(
                        angle: _driverHeading != null
                            ? (_driverHeading! + 90) * pi / 180
                            : 0.0,
                        child: Image.asset('assets/images/taxi_top.png', width: 36, height: 36),
                      ),
                    ),
                  ],
                ),
              // Aktif yolculuk pickup/dropoff marker'ları
              if (_hasActiveRide && _activeRideInfo != null) ...[
                MarkerLayer(
                  markers: [
                    if (_activeRideInfo!['pickup_lat'] != null &&
                        _activeRideInfo!['pickup_lon'] != null)
                      Marker(
                        point: LatLng(
                          (_activeRideInfo!['pickup_lat'] as num).toDouble(),
                          (_activeRideInfo!['pickup_lon'] as num).toDouble(),
                        ),
                        width: 40,
                        height: 40,
child: Image.asset('assets/images/cheer-up.png', width: 32, height: 32),
                       ),
                     if (_activeRideInfo!['dropoff_lat'] != null &&
                        _activeRideInfo!['dropoff_lon'] != null)
                      Marker(
                        point: LatLng(
                          (_activeRideInfo!['dropoff_lat'] as num).toDouble(),
                          (_activeRideInfo!['dropoff_lon'] as num).toDouble(),
                        ),
                        width: 40,
                        height: 40,
                        child: const Icon(
                          Icons.location_on,
                          color: Colors.red,
                          size: 32,
                        ),
                      ),
                  ],
                ),
              ],
              // Premium: teklif önizleme marker'ları (kabul etmeden önce)
              if (_userMapEnabled && _pendingOffer != null) ...[
                MarkerLayer(
                  markers: [
                    if (_pendingOffer!['pickup_lat'] != null &&
                        _pendingOffer!['pickup_lon'] != null)
                      Marker(
                        point: LatLng(
                          (_pendingOffer!['pickup_lat'] as num).toDouble(),
                          (_pendingOffer!['pickup_lon'] as num).toDouble(),
                        ),
                        width: 40,
                        height: 40,
                        child: Image.asset('assets/images/cheer-up.png', width: 32, height: 32),
                       ),
                     if (_pendingOffer!['dropoff_lat'] != null &&
                        _pendingOffer!['dropoff_lon'] != null)
                      Marker(
                        point: LatLng(
                          (_pendingOffer!['dropoff_lat'] as num).toDouble(),
                          (_pendingOffer!['dropoff_lon'] as num).toDouble(),
                        ),
                        width: 40,
                        height: 40,
                        child: const Icon(
                          Icons.location_on,
                          color: Colors.red,
                          size: 32,
                        ),
                      ),
                  ],
                ),
              ],
            ],
          ),

          // Üst bar — On/Off sol tarafta, Çıkış sağ tarafta
          Positioned(
            top: topPadding + 8,
            left: 16,
            child: GestureDetector(
              onTap: _toggleOnline,
              child: Container(
                width: 44,
                height: 44,
                decoration: BoxDecoration(
                  color: _isOnline ? Colors.green : Colors.red,
                  shape: BoxShape.circle,
                  boxShadow: [
                    BoxShadow(
                      color: (_isOnline ? Colors.green : Colors.red).withAlpha(80),
                      blurRadius: 8,
                    ),
                  ],
                ),
                child: Icon(
                  Icons.power_settings_new,
                  color: Colors.white,
                  size: 24,
                ),
              ),
            ),
          ),
          Positioned(
            top: topPadding + 8,
            right: 16,
            child: Container(
              decoration: BoxDecoration(
                color: Colors.white,
                shape: BoxShape.circle,
                boxShadow: [
                  BoxShadow(
                    color: Colors.black.withAlpha(40),
                    blurRadius: 8,
                    offset: const Offset(0, 2),
                  ),
                ],
              ),
              child: IconButton(
                icon: const Icon(Icons.menu, size: 24),
                onPressed: () {
                  _scaffoldKey.currentState?.openDrawer();
                },
              ),
            ),
          ),

          // Aktif yolculuk bilgi kartı
          if (_hasActiveRide && _activeRideInfo != null && _pendingOffer == null)
            Positioned(
              bottom: MediaQuery.of(context).padding.bottom,
              left: 0,
              right: 0,
              child: Container(
                margin: const EdgeInsets.all(16),
                padding: const EdgeInsets.all(20),
                decoration: BoxDecoration(
                  color: Colors.white,
                  borderRadius: BorderRadius.circular(20),
                  boxShadow: [
                    BoxShadow(
                      color: Colors.black.withAlpha(50),
                      blurRadius: 20,
                      offset: const Offset(0, -4),
                    ),
                  ],
                ),
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Row(
                      children: [
                        Icon(_ridePhase == 'picked_up' ? Icons.local_taxi : Icons.directions_car, color: _ridePhase == 'picked_up' ? Colors.blue : Colors.green, size: 24),
                        const SizedBox(width: 8),
                        Text(
                          _ridePhase == 'picked_up' ? 'Yolculuk başladı' : 'Sürücü yolda',
                          style: TextStyle(
                            fontSize: 16,
                            fontWeight: FontWeight.bold,
                            color: _ridePhase == 'picked_up' ? Colors.blue : Colors.green,
                          ),
                        ),
                      ],
                    ),
                    const SizedBox(height: 8),
                    // Sürücü → Pickup mesafe/süre
                    if (_ridePhase != 'picked_up' && (_pickupRouteKm != null || _pickupRouteMin != null))
                      Padding(
                        padding: const EdgeInsets.only(bottom: 4),
                        child: Row(
                          mainAxisAlignment: MainAxisAlignment.spaceBetween,
                          children: [
                            Row(
                              children: [
                                const Icon(Icons.directions_car, size: 14, color: Colors.green),
                                const SizedBox(width: 4),
                                const Text('Sürücü → Alınma', style: TextStyle(fontSize: 13, color: Colors.green)),
                              ],
                            ),
                            Row(
                              children: [
                                if (_pickupRouteKm != null)
                                  Text('${_pickupRouteKm!.toStringAsFixed(1)} km', style: const TextStyle(fontWeight: FontWeight.bold, fontSize: 13)),
                                if (_pickupRouteKm != null && _pickupRouteMin != null)
                                  const SizedBox(width: 4),
                                if (_pickupRouteMin != null)
                                  Text('• $_pickupRouteMin dk', style: TextStyle(color: Colors.grey[600], fontSize: 13)),
                              ],
                            ),
                          ],
                        ),
                      ),
                    // Pickup → Dropoff mesafe/süre
                    if (_activeRideInfo!['distance_km'] != null || _activeRideInfo!['duration_sec'] != null)
                      Padding(
                        padding: const EdgeInsets.only(bottom: 4),
                        child: Row(
                          mainAxisAlignment: MainAxisAlignment.spaceBetween,
                          children: [
                            Row(
                              children: [
                                const Icon(Icons.route, size: 14, color: Colors.blue),
                                const SizedBox(width: 4),
                                const Text('Yolculuk rotası', style: TextStyle(fontSize: 13, color: Colors.blue)),
                              ],
                            ),
                            Row(
                              children: [
                                if (_activeRideInfo!['distance_km'] != null)
                                  Text('${(_activeRideInfo!['distance_km'] as num).toStringAsFixed(1)} km', style: const TextStyle(fontWeight: FontWeight.bold, fontSize: 13)),
                                if (_activeRideInfo!['distance_km'] != null && _activeRideInfo!['duration_sec'] != null)
                                  const SizedBox(width: 4),
                                if (_activeRideInfo!['duration_sec'] != null) ...[
                                  Text('• ${_formatDuration((_activeRideInfo!['duration_sec'] as num).round())}', style: TextStyle(color: Colors.grey[600], fontSize: 13)),
                                ],
                              ],
                            ),
                          ],
                        ),
                      ),
                    const SizedBox(height: 8),
                    // Başlangıç ve bitiş — yan yana
                    Row(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Expanded(
                          child: Row(
                            children: [
                              const Icon(Icons.circle, color: Colors.green, size: 14),
                              const SizedBox(width: 6),
                              Expanded(
                                child: Text(
                                  _activeRideInfo!['pickup_address'] as String? ?? 'Alınma noktası',
                                  style: const TextStyle(fontSize: 13),
                                ),
                              ),
                            ],
                          ),
                        ),
                        const SizedBox(width: 8),
                        Expanded(
                          child: Row(
                            children: [
                              const Icon(Icons.location_on, color: Colors.red, size: 14),
                              const SizedBox(width: 6),
                              Expanded(
                                child: Text(
                                  _activeRideInfo!['dropoff_address'] as String? ?? 'Bırakılma noktası',
                                  style: const TextStyle(fontSize: 13),
                                ),
                              ),
                            ],
                          ),
                        ),
                      ],
                    ),
                    if (_activeRideInfo!['fare_amount'] != null) ...[
                      const SizedBox(height: 10),
                      const Divider(height: 1),
                      const SizedBox(height: 10),
                      _buildDriverFareSection(_activeRideInfo!),
                    ],
                    const SizedBox(height: 16),
                    if (_ridePhase == 'picked_up') ...[
                      SizedBox(
                        width: double.infinity,
                        child: ElevatedButton.icon(
                          onPressed: _completeRide,
                          icon: const Icon(Icons.check_circle, color: Colors.white),
                          label: const Text(
                            'Yolculuğu Bitir',
                            style: TextStyle(fontSize: 16, fontWeight: FontWeight.bold),
                          ),
                          style: ElevatedButton.styleFrom(
                            backgroundColor: Colors.red,
                            foregroundColor: Colors.white,
                            padding: const EdgeInsets.symmetric(vertical: 14),
                            shape: RoundedRectangleBorder(
                              borderRadius: BorderRadius.circular(10),
                            ),
                          ),
                        ),
                      ),
                    ] else ...[
                      Row(
                        children: [
                          Expanded(
                            child: OutlinedButton.icon(
                              onPressed: () async {
                                final rideId = _activeRideInfo!['ride_id'] as int;
                                await RideService.updateRideStatus(rideId, 'cancelled');
                                _endActiveRide('cancelled');
                              },
                              icon: const Icon(Icons.cancel_outlined, size: 18),
                              label: const Text('İptal'),
                              style: OutlinedButton.styleFrom(
                                foregroundColor: Colors.red,
                                side: const BorderSide(color: Colors.red),
                                padding: const EdgeInsets.symmetric(vertical: 14),
                                shape: RoundedRectangleBorder(
                                  borderRadius: BorderRadius.circular(10),
                                ),
                              ),
                            ),
                          ),
                          const SizedBox(width: 12),
                          Expanded(
                            flex: 2,
                            child: ElevatedButton.icon(
                              onPressed: _pickUpPassenger,
                              icon: const Icon(Icons.person_add, color: Colors.white),
                              label: const Text(
                                'Yolcuyu Aldım',
                                style: TextStyle(fontSize: 16, fontWeight: FontWeight.bold),
                              ),
                              style: ElevatedButton.styleFrom(
                                backgroundColor: Colors.green,
                                foregroundColor: Colors.white,
                                padding: const EdgeInsets.symmetric(vertical: 14),
                                shape: RoundedRectangleBorder(
                                  borderRadius: BorderRadius.circular(10),
                                ),
                              ),
                            ),
                          ),
                        ],
                      ),
                    ],
                  ],
                ),
              ),
            ),
          if (_pendingOffer != null)
            Positioned(
              bottom: MediaQuery.of(context).padding.bottom,
              left: 0,
              right: 0,
              child: Container(
                margin: const EdgeInsets.all(16),
                padding: const EdgeInsets.all(20),
                decoration: BoxDecoration(
                  color: Colors.white,
                  borderRadius: BorderRadius.circular(20),
                  boxShadow: [
                    BoxShadow(
                      color: Colors.black.withAlpha(50),
                      blurRadius: 20,
                      offset: const Offset(0, -4),
                    ),
                  ],
                ),
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Row(
                      children: [
                        const Icon(Icons.local_taxi, color: Colors.amber, size: 24),
                        const SizedBox(width: 8),
                        const Text(
                          'Yeni Yolculuk Teklifi',
                          style: TextStyle(
                            fontSize: 16,
                            fontWeight: FontWeight.bold,
                          ),
                        ),
                        const Spacer(),
                        Container(
                          width: 36,
                          height: 36,
                          decoration: BoxDecoration(
                            shape: BoxShape.circle,
                            border: Border.all(color: Colors.orange, width: 2),
                          ),
                          child: Center(
                            child: Text(
                              '$_offerCountdown',
                              style: const TextStyle(
                                fontWeight: FontWeight.bold,
                                fontSize: 14,
                                color: Colors.orange,
                              ),
                            ),
                          ),
                        ),
                      ],
                    ),
                    const SizedBox(height: 8),
                    // Sürücü → Alınma noktası mesafe/süre
                    if (_pickupRouteKm != null || _pickupRouteMin != null)
                      Padding(
                        padding: const EdgeInsets.only(bottom: 4),
                        child: Row(
                          mainAxisAlignment: MainAxisAlignment.spaceBetween,
                          children: [
                            Row(
                              children: [
                                const Icon(Icons.directions_car, size: 14, color: Colors.green),
                                const SizedBox(width: 4),
                                const Text('Sürücü → Alınma', style: TextStyle(fontSize: 13, color: Colors.green)),
                              ],
                            ),
                            Row(
                              children: [
                                if (_pickupRouteKm != null)
                                  Text('${_pickupRouteKm!.toStringAsFixed(1)} km', style: const TextStyle(fontWeight: FontWeight.bold, fontSize: 13)),
                                if (_pickupRouteKm != null && _pickupRouteMin != null)
                                  const SizedBox(width: 4),
                                if (_pickupRouteMin != null)
                                  Text('• $_pickupRouteMin ${_pickupRouteMin! >= 60 ? '' : ''}dk', style: TextStyle(color: Colors.grey[600], fontSize: 13)),
                              ],
                            ),
                          ],
                        ),
                      ),
                    // Yolculuk rotası mesafe/süre
                    if (_pendingOffer!['distance_km'] != null || _pendingOffer!['duration_sec'] != null)
                      Padding(
                        padding: const EdgeInsets.only(bottom: 4),
                        child: Row(
                          mainAxisAlignment: MainAxisAlignment.spaceBetween,
                          children: [
                            Row(
                              children: [
                                const Icon(Icons.route, size: 14, color: Colors.blue),
                                const SizedBox(width: 4),
                                const Text('Yolculuk rotası', style: TextStyle(fontSize: 13, color: Colors.blue)),
                              ],
                            ),
                            Row(
                              children: [
                                if (_pendingOffer!['distance_km'] != null)
                                  Text('${(_pendingOffer!['distance_km'] as num).toStringAsFixed(1)} km', style: const TextStyle(fontWeight: FontWeight.bold, fontSize: 13)),
                                if (_pendingOffer!['distance_km'] != null && _pendingOffer!['duration_sec'] != null)
                                  const SizedBox(width: 4),
                                if (_pendingOffer!['duration_sec'] != null)
                                  Text('• ${_formatDuration((_pendingOffer!['duration_sec'] as num).round())}', style: TextStyle(color: Colors.grey[600], fontSize: 13)),
                              ],
                            ),
                          ],
                        ),
                      ),
                    const SizedBox(height: 8),
                    // Başlangıç ve bitiş — yan yana
                    Row(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Expanded(
                          child: Row(
                            children: [
                              const Icon(Icons.circle, color: Colors.green, size: 14),
                              const SizedBox(width: 6),
                              Expanded(
                                child: Text(
                                  _pendingOffer!['pickup_address'] as String? ?? '?',
                                  style: const TextStyle(fontSize: 13),
                                ),
                              ),
                            ],
                          ),
                        ),
                        const SizedBox(width: 8),
                        Expanded(
                          child: Row(
                            children: [
                              const Icon(Icons.location_on, color: Colors.red, size: 14),
                              const SizedBox(width: 6),
                              Expanded(
                                child: Text(
                                  _pendingOffer!['dropoff_address'] as String? ?? '?',
                                  style: const TextStyle(fontSize: 13),
                                ),
                              ),
                            ],
                          ),
                        ),
                      ],
                    ),
                    // Ücret detayı — teklif anında göster
                    if (_pendingOffer!['fare_info'] != null ||
                        _pendingOffer!['fare_amount'] != null) ...[
                      const SizedBox(height: 10),
                      const Divider(height: 1),
                      const SizedBox(height: 10),
                      _buildDriverFareSection(_pendingOffer!),
                    ],
                    const SizedBox(height: 20),
                    Row(
                      children: [
                        Expanded(
                          child: OutlinedButton(
                            onPressed: _rejectOffer,
                            style: OutlinedButton.styleFrom(
                              foregroundColor: Colors.red,
                              side: const BorderSide(color: Colors.red),
                              padding: const EdgeInsets.symmetric(vertical: 14),
                              shape: RoundedRectangleBorder(
                                borderRadius: BorderRadius.circular(10),
                              ),
                            ),
                            child: const Text(
                              'Reddet',
                              style: TextStyle(fontSize: 16, fontWeight: FontWeight.bold),
                            ),
                          ),
                        ),
                        const SizedBox(width: 12),
                        Expanded(
                          flex: 2,
                          child: ElevatedButton(
                            onPressed: _acceptOffer,
                            style: ElevatedButton.styleFrom(
                              backgroundColor: Colors.green,
                              foregroundColor: Colors.white,
                              padding: const EdgeInsets.symmetric(vertical: 14),
                              shape: RoundedRectangleBorder(
                                borderRadius: BorderRadius.circular(10),
                              ),
                            ),
                            child: const Text(
                              'Kabul Et',
                              style: TextStyle(fontSize: 16, fontWeight: FontWeight.bold),
                            ),
                          ),
                        ),
                      ],
                    ),
                  ],
                ),
              ),
            ),
        ],
      ),
    );
  }

  Widget _buildDrawer(BuildContext context) {
    final user = ref.watch(authProvider).user;
    final isAuthenticated = ref.watch(authProvider).isAuthenticated;
    final primary = Theme.of(context).colorScheme.primary;

    return Drawer(
      child: SafeArea(
        child: Column(
          children: [
            Container(
              width: double.infinity,
              padding: const EdgeInsets.fromLTRB(20, 24, 20, 20),
              decoration: BoxDecoration(
                color: primary.withAlpha(15),
              ),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  CircleAvatar(
                    radius: 30,
                    backgroundColor: primary,
                    child: Icon(
                      Icons.local_taxi,
                      color: Colors.white,
                      size: 30,
                    ),
                  ),
                  const SizedBox(height: 12),
                  Text(
                    isAuthenticated && user != null
                        ? user.fullName
                        : 'Sürücü',
                    style: const TextStyle(
                      fontSize: 18,
                      fontWeight: FontWeight.bold,
                    ),
                  ),
                  if (isAuthenticated && user != null)
                    Text(
                      '@${user.username}',
                      style: TextStyle(
                        fontSize: 14,
                        color: Colors.grey[600],
                      ),
                    ),
                ],
              ),
            ),
            const SizedBox(height: 8),
            ListTile(
              leading: const Icon(Icons.person_outline),
              title: const Text('Profilim'),
              onTap: () {
                Navigator.of(context).pop();
                if (isAuthenticated) {
                  context.go('/profile');
                } else {
                  context.go('/login');
                }
              },
            ),
            ListTile(
              leading: const Icon(Icons.history),
              title: const Text('Geçmiş Yolculuklar'),
              onTap: () {
                Navigator.of(context).pop();
                if (isAuthenticated) {
                  context.go('/rideHistory');
                } else {
                  context.go('/login');
                }
              },
            ),
            ListTile(
              leading: const Icon(Icons.settings_outlined),
              title: const Text('Ayarlar'),
              onTap: () {
                Navigator.of(context).pop();
                context.go('/settings');
              },
            ),
            const Divider(),
            if (isAuthenticated)
              ListTile(
                leading: const Icon(Icons.logout, color: Colors.red),
                title: const Text(
                  'Çıkış Yap',
                  style: TextStyle(color: Colors.red),
                ),
                onTap: () async {
                  Navigator.of(context).pop();
                  await ref.read(authProvider.notifier).logout();
                },
              )
            else
              ListTile(
                leading: Icon(Icons.login, color: primary),
                title: Text(
                  'Giriş Yap',
                  style: TextStyle(color: primary),
                ),
                onTap: () {
                  Navigator.of(context).pop();
                  context.go('/login');
                },
              ),
          ],
        ),
      ),
    );
  }
}