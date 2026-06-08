import 'dart:async';
import 'dart:math';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:latlong2/latlong.dart';
import 'package:geolocator/geolocator.dart';
import 'package:go_router/go_router.dart';
import 'package:url_launcher/url_launcher.dart';
import '../auth/providers/auth_provider.dart';
import 'providers/ride_provider.dart';
import 'services/route_service.dart' show RouteService, FareInfo, SearchResult;
import 'services/ride_service.dart' show RideService, ActiveRideInfo, LocationSearchApi;

/// Ana taksi çağırma ekranı.
///
/// Harita tabanlı UI; kullanıcı hedef arar, rota görür, taksi çağırır,
/// sürücü konumunu canlı takip eder ve yolculuk durumuna göre kart gösterir.
///
/// Backend ilişkisi:
/// - `requestRide()` → POST /api/ride/request (yeni yolculuk oluşturur)
/// - `cancelRide()`   → POST /api/ride/:id/cancel (yolculuğu iptal eder)
/// - `completeRide()` → POST /api/ride/:id/status {status: completed}
/// - WebSocket aracılığıyla sürücü konumu ve durum değişiklikleri alınır
/// - `RouteService.getRoute()` → ORS API (rota hesaplama, backend proxy'siz)
class RideHomePage extends ConsumerStatefulWidget {
  const RideHomePage({super.key});

  @override
  ConsumerState<RideHomePage> createState() => _RideHomePageState();
}

/// RideHomePage'ın state sınıfı.
///
/// Harita kontrolü, arama overlay yönetimi, GPS konumu, kamera takibi,
/// ve ride durumuna göre UI kartı oluşturma burada yönetilir.
class _RideHomePageState extends ConsumerState<RideHomePage> {
  /// Hedef arama TextField'ı kontrol eder. Kullanıcının yazdığı metin burada.
  /// idle dışındaki durumlarda readOnly olur.
  final TextEditingController _destinationController = TextEditingController();

  /// flutter_map harita kontrolcüsü. Kamera hareketleri ve zoom için.
  final MapController _mapController = MapController();

  /// Arama sonuçları dropdown'unu konumlandırmak için CompositedTransformTarget linki.
  final LayerLink _layerLink = LayerLink();

  /// Arama sonuçları overlay girdisi. Açılıp kapatma lifecycle'ı yönetilir.
  OverlayEntry? _overlayEntry;

  /// Arama kutusunda yazma debounce timer'ı. 300ms gecikme ile API çağrısı yapar.
  Timer? _debounceTimer;

  /// GPS konumu henüz alınmamışsa true, alınca false olur.
  bool _isInitializing = true;

  /// Kullanıcı haritayı elle kaydırdığında false olur,
  /// sürücü konumu güncellenince picked_up/accepted durumunda true olur.
  bool _cameraFollowing = false;

  /// Yakındaki sürücüleri çekerken debounce timer'ı.
  Timer? _nearbyDriversTimer;

  /// Varsayılan harita merkezi — Sakarya Serdivan.
  static const LatLng _defaultCenter = LatLng(40.7604062, 30.3629614);

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      _initLocation();
      _restoreSearchText();
      _checkAndRestoreActiveRide();
    });
  }

  /// Sayfa yeniden açıldığında, rideProvider'daki destinationAddress
  /// ile TextField'ı senkronize eder (ör. "Tekrar Dene" sonrası).
  void _restoreSearchText() {
    final rideState = ref.read(rideProvider);
    if (rideState.destinationAddress != null) {
      _destinationController.text = rideState.destinationAddress!;
    }
  }

  /// Uygulama açılınca backend'den yolcunun aktif ride kontrolü yapar.
  /// Varsa kullanıcıya sorar: Devam, Tamamlandı, İptal.
  Future<void> _checkAndRestoreActiveRide() async {
    final activeRide = await RideService.getPassengerActiveRide();
    if (activeRide == null) return;
    if (!mounted) return;

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
        await _resolvePassengerRide(activeRide.rideId, 'cancelled');
      case 'continue':
        await _continuePassengerRide(activeRide);
      case 'completed':
        await _resolvePassengerRide(activeRide.rideId, 'completed');
    }
  }

  /// Yolcunun yarım kalan ride'ını tamamla veya iptal et.
  Future<void> _resolvePassengerRide(int rideId, String resolution) async {
    if (resolution == 'completed') {
      await RideService.updateRideStatus(rideId, 'completed');
    } else {
      await RideService.cancelRide(rideId, by: 'passenger');
    }
    if (!mounted) return;
    ref.read(rideProvider.notifier).resetRide();
    _destinationController.clear();
    _removeOverlay();
    setState(() {});
  }

  /// Yolcunun yarım kalan ride'ını devam ettir (WS yeniden bağlanır, rota çizer).
  Future<void> _continuePassengerRide(ActiveRideInfo activeRide) async {
    debugPrint('PassengerHomePage: Yarım kalan ride #${activeRide.rideId} devam ediyor (${activeRide.status})');

    ref.read(rideProvider.notifier).restoreActiveRide(
      rideId: activeRide.rideId,
      status: activeRide.status,
      pickupAddress: activeRide.pickupAddress,
      dropoffAddress: activeRide.dropoffAddress,
      pickupLat: activeRide.pickupLat,
      pickupLon: activeRide.pickupLon,
      dropoffLat: activeRide.dropoffLat,
      dropoffLon: activeRide.dropoffLon,
      distanceKm: activeRide.distanceKm,
      durationSec: activeRide.durationSec,
    );

    // Rota çizgisini çek
    try {
      final result = await RouteService.getRoute(
        LatLng(activeRide.pickupLat, activeRide.pickupLon),
        LatLng(activeRide.dropoffLat, activeRide.dropoffLon),
      );
      if (result.points.isNotEmpty) {
        ref.read(rideProvider.notifier).setRoute(
          result.points,
          result,
        );
      }
    } catch (e) {
      debugPrint('PassengerHomePage: rota çekilemedi: $e');
    }

    if (!mounted) return;
    _showAutoDialog(
      kind: 'success',
      title: 'Yolculuk Devam Ediyor',
      desc: activeRide.status == 'picked_up'
          ? 'Yolculuğunuz devam ediyor. Varış noktasına ilerleniyor.'
          : 'Sürücü yolda. Pick-up noktasına ilerleniyor.',
    );
  }

  void _showAutoDialog({
    required String kind,
    required String title,
    required String desc,
  }) {
    if (!mounted) return;
    Color color;
    IconData icon;
    if (kind == 'success') {
      color = Colors.green;
      icon = Icons.check_circle;
    } else if (kind == 'warning') {
      color = Colors.orange;
      icon = Icons.warning_amber;
    } else if (kind == 'error') {
      color = Colors.red;
      icon = Icons.error;
    } else {
      color = Colors.blue;
      icon = Icons.info;
    }
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

@override
  void dispose() {
    _destinationController.dispose();
    _debounceTimer?.cancel();
    _nearbyDriversTimer?.cancel();
    _overlayEntry?.remove();
    super.dispose();
  }

  /// GPS'ten kullanıcının mevcut konumunu alır ve rideProvider'a yazar.
  ///
  /// 1. Önce rideProvider'da zaten konum varsa tekrar istemez.
  /// 2. İzin verilmediyse veya hata olursa varsayılan konumu (_defaultCenter) kullanır.
  /// 3. Backend ile doğrudan ilişkisi yoktur; konum yalnızca client tarafında tutulur,
  ///    requestRide() sırasında pickup_lat/pickup_lon olarak backend'e gönderilir.
  Future<void> _initLocation() async {
    final rideState = ref.read(rideProvider);

    // Eğer zaten bir konum varsa kullan
    if (rideState.currentLocation != null) {
      setState(() => _isInitializing = false);
      return;
    }

    // Aksi halde GPS'ten al
    try {
      LocationPermission permission = await Geolocator.checkPermission();
      if (permission == LocationPermission.denied) {
        permission = await Geolocator.requestPermission();
      }

      if (permission == LocationPermission.denied ||
          permission == LocationPermission.deniedForever) {
        ref.read(rideProvider.notifier).setCurrentLocation(_defaultCenter);
        setState(() => _isInitializing = false);
        return;
      }

      final position = await Geolocator.getCurrentPosition(
        locationSettings: const LocationSettings(
          accuracy: LocationAccuracy.high,
          timeLimit: Duration(seconds: 5),
        ),
      );

      final location = LatLng(position.latitude, position.longitude);
      ref.read(rideProvider.notifier).setCurrentLocation(location);
    } catch (e) {
      ref.read(rideProvider.notifier).setCurrentLocation(_defaultCenter);
    } finally {
      if (mounted) {
        setState(() => _isInitializing = false);
      }
    }
  }

  /// GPS'ten taze konum alır, provider'ı günceller ve haritayı oraya ortalar.
  Future<void> _refreshLocation() async {
    try {
      final position = await Geolocator.getCurrentPosition(
        locationSettings: const LocationSettings(
          accuracy: LocationAccuracy.high,
          timeLimit: Duration(seconds: 5),
        ),
      );
      final location = LatLng(position.latitude, position.longitude);
      ref.read(rideProvider.notifier).setCurrentLocation(location);
      _mapController.move(location, 16);
    } catch (e) {
      // GPS başarısız olursa mevcut konumu dene
      final existing = ref.read(rideProvider).currentLocation;
      if (existing != null) {
        _mapController.move(existing, 16);
      }
    }
  }

  /// Açık olan arama sonuçları overlay'ini kaldırır.
  void _removeOverlay() {
    _overlayEntry?.remove();
    _overlayEntry = null;
  }

  /// Arama sonuçlarını ekranda dropdown olarak gösterir.
  ///
  /// [results] → LocationSearchApi.search()'ten gelen DB + Nominatim sonuçları.
  /// Kullanıcı bir sonuca tıklayınca _selectDestination() çağrılır.
  void _showOverlay(List<SearchResult> results) {
    _removeOverlay();

    _overlayEntry = OverlayEntry(
      builder: (context) => Positioned(
        width: MediaQuery.of(context).size.width - 32,
        child: CompositedTransformFollower(
          link: _layerLink,
          showWhenUnlinked: false,
          offset: const Offset(0, 55),
          child: Material(
            elevation: 4,
            borderRadius: BorderRadius.circular(12),
            child: ConstrainedBox(
              constraints: const BoxConstraints(maxHeight: 200),
              child: ListView.builder(
                padding: EdgeInsets.zero,
                shrinkWrap: true,
                itemCount: results.length,
                itemBuilder: (context, index) {
                  final result = results[index];
                  final isDb = result.source == 'db';
                  return ListTile(
                    leading: Icon(
                      isDb ? Icons.bookmark : Icons.location_on,
                      color: isDb ? Theme.of(context).colorScheme.primary : Colors.red,
                      size: isDb ? 22 : 24,
                    ),
                    title: Text(
                      result.displayName,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                    ),
                    onTap: () {
                      _destinationController.text = result.displayName;
                      FocusManager.instance.primaryFocus?.unfocus();
                      _removeOverlay();
                      _selectDestination(result.coordinate, result.displayName);
                    },
                  );
                },
              ),
            ),
          ),
        ),
      ),
    );

    Overlay.of(context).insert(_overlayEntry!);
  }

  /// Arama kutusunda her tuş vuruşunda çağrılır (debounce ile).
  ///
  /// Önce backend DB'de arar → kaydedilmiş mekanlar çıkar.
  /// Sonuç yetersizse ORS Nominatim fallback devreye girer.
  void _onSearchChanged(String value) {
    _debounceTimer?.cancel();

    if (value.trim().length < 2) {
      _removeOverlay();
      return;
    }

    _debounceTimer = Timer(const Duration(milliseconds: 300), () async {
      try {
        final results = await LocationSearchApi.search(value.trim());

        if (results.isNotEmpty) {
          _showOverlay(results);
        } else {
          _removeOverlay();
        }
      } catch (e) {
        _removeOverlay();
      }
    });
  }

  /// Kullanıcı hedef seçtiğinde çağrılır (autocomplete veya harita tap).
  ///
  /// 1. rideProvider'a hedef koordinat ve adres yazar.
  /// 2. RouteService.getRoute() ile ORS Driving API'den rota hesaplar.
  /// 3. Rota noktalarını ve mesafe/süre bilgisini rideProvider'a yazar.
  /// 4. Haritayı rotaya sığacak şekilde yakınlaştırır.
  ///
  /// Backend ilişkisi: Rota hesaplama ORS API'ye doğrudan gider (backend proxy'siz).
  /// Backend'e bu rota gönderilmez; yalnızca requestRide() sırasında
  /// pickup/dropoff koordinatları gönderilir, backend kendi rota hesaplamasını yapar.
  Future<void> _selectDestination(LatLng destination, String address) async {
    ref.read(rideProvider.notifier).setDestination(destination, address);

    final rideState = ref.read(rideProvider);
    final currentLocation = rideState.currentLocation ?? _defaultCenter;

    try {
      final result = await RouteService.getRoute(currentLocation, destination);

      ref.read(rideProvider.notifier).setRoute(result.points, result);

      if (result.points.isNotEmpty) {
        final bounds = LatLngBounds.fromPoints(result.points);
        _mapController.fitCamera(
          CameraFit.bounds(bounds: bounds, padding: const EdgeInsets.all(50)),
        );
      }
    } catch (e) {
      if (mounted) {
        showDialog(
          context: context,
          builder: (ctx) => AlertDialog(
            title: const Row(children: [Icon(Icons.warning_amber, color: Colors.orange), SizedBox(width: 8), Text('Rota Hesaplanamadı')]),
            content: const Text('Lütfen tekrar deneyin.'),
            actions: [TextButton(onPressed: () => Navigator.of(ctx).pop(), child: const Text('Tamam'))],
          ),
        );
      }
    }
  }

  /// Haritaya dokunulduğunda çağrılır.
  ///
  /// Sadece idle durumunda çalışır (aktif yolculukta haritayı kilitler).
  /// Dokunulan noktayı mevcut konum olarak rideProvider'a yazar,
  /// eğer önceden bir hedef seçilmişse rotayı yeniden hesaplar.
  void _onMapTap(TapPosition tapPosition, LatLng point) {
    final rideState = ref.read(rideProvider);
    if (rideState.rideStatus != 'idle') return;

    ref.read(rideProvider.notifier).setCurrentLocation(point);

    if (rideState.destination != null) {
      _selectDestination(
        rideState.destination!,
        rideState.destinationAddress ?? '',
      );
    }
  }

  /// Haritanın görünen alanı değiştiğinde yakındaki sürücüleri çeker.
  void _onCameraMoved() {
    _nearbyDriversTimer?.cancel();
    _nearbyDriversTimer = Timer(const Duration(milliseconds: 500), () {
      final bounds = _mapController.camera.visibleBounds;
      ref.read(rideProvider.notifier).fetchNearbyDrivers(
        minLat: bounds.south,
        maxLat: bounds.north,
        minLon: bounds.west,
        maxLon: bounds.east,
      );
    });
  }

  /// Ride durumuna göre "Taksi Çağır" butonunun etiketini döner.
  /// Yolculuğu iptal eder ve UI'ı temizler.
  ///
  /// Backend → POST /api/ride/:id/cancel
  /// Sonra destination TextField, overlay ve state temizlenir.
  Future<void> _cancelRide() async {
    await ref.read(rideProvider.notifier).cancelRide();
    _destinationController.clear();
    _removeOverlay();
    setState(() {});
  }

  final GlobalKey<ScaffoldState> _scaffoldKey = GlobalKey<ScaffoldState>();

  @override
  Widget build(BuildContext context) {
    final authState = ref.watch(authProvider);
    final isAuthenticated = authState.isAuthenticated;
    final user = authState.user;
    final rideState = ref.watch(rideProvider);
    final topPadding = MediaQuery.of(context).padding.top;

    // Sürücü konumu değişince haritayı kaydır (sadece takip modu açıkken)
    ref.listen<RideState>(rideProvider, (prev, next) {
      // accepted veya picked_up olunca kamerayı otomatik takibe al
      if ((next.rideStatus == 'accepted' || next.rideStatus == 'picked_up') &&
          prev?.rideStatus != 'accepted' && prev?.rideStatus != 'picked_up') {
        _cameraFollowing = true;
      }
      // Sürücü konumu değişince haritayı kaydır
      if (next.driverLocation != null &&
          next.driverLocation != prev?.driverLocation &&
          _cameraFollowing &&
          (next.rideStatus == 'accepted' || next.rideStatus == 'picked_up')) {
        _mapController.move(next.driverLocation!, _mapController.camera.zoom);
      }
    });

    return Scaffold(
      key: _scaffoldKey,
      drawer: Drawer(
        child: SafeArea(
          child: Column(
            children: [
              // Drawer header
              Container(
                width: double.infinity,
                padding: const EdgeInsets.fromLTRB(20, 24, 20, 20),
                decoration: BoxDecoration(
                  color: Theme.of(context).colorScheme.primary.withAlpha(15),
                ),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    CircleAvatar(
                      radius: 30,
                      backgroundColor: Theme.of(context).colorScheme.primary,
                      child: Icon(
                        isAuthenticated ? Icons.person : Icons.person_outline,
                        color: Colors.white,
                        size: 30,
                      ),
                    ),
                    const SizedBox(height: 12),
                    Text(
                      isAuthenticated && user != null
                          ? user.fullName
                          : 'Misafir',
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
              // Menu items
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
                  leading: Icon(
                    Icons.login,
                    color: Theme.of(context).colorScheme.primary,
                  ),
                  title: Text(
                    'Giriş Yap',
                    style: TextStyle(
                      color: Theme.of(context).colorScheme.primary,
                    ),
                  ),
                  onTap: () {
                    Navigator.of(context).pop();
                    context.go('/login');
                  },
                ),
            ],
          ),
        ),
      ),
      body: Stack(
        children: [
          // Harita - tam ekran
          FlutterMap(
            mapController: _mapController,
            options: MapOptions(
              initialCenter: rideState.currentLocation ?? _defaultCenter,
              initialZoom: 14,
              onTap: _onMapTap,
              onPositionChanged: (pos, hasGesture) {
                if (hasGesture) _cameraFollowing = false;
                _onCameraMoved();
              },
              onMapReady: () {
                // Harita hazır olduğunda rotayı göster
                if (rideState.routePoints.isNotEmpty &&
                    rideState.destination != null) {
                  WidgetsBinding.instance.addPostFrameCallback((_) {
                    if (mounted) {
                      final bounds = LatLngBounds.fromPoints(
                        rideState.routePoints,
                      );
                      _mapController.fitCamera(
                        CameraFit.bounds(
                          bounds: bounds,
                          padding: const EdgeInsets.all(50),
                        ),
                      );
                    }
                  });
                }
              },
            ),
            children: [
              TileLayer(
                urlTemplate: 'https://tile.openstreetmap.org/{z}/{x}/{y}.png',
                userAgentPackageName: 'com.example.ride_rs',
              ),
              // Başlangıç noktası (turuncu pin)
              if (rideState.currentLocation != null)
                MarkerLayer(
                  markers: [
                    Marker(
                      point: rideState.currentLocation!,
                      width: 40,
                      height: 40,
                      child: const Icon(
                        Icons.person_pin_circle,
                        color: Colors.deepOrange,
                        size: 40,
                      ),
                    ),
                  ],
                ),
              // Hedef noktası (kırmızı)
              if (rideState.destination != null)
                MarkerLayer(
                  markers: [
                    Marker(
                      point: rideState.destination!,
                      width: 40,
                      height: 40,
                      child: const Icon(
                        Icons.location_on,
                        color: Colors.red,
                        size: 40,
                      ),
                    ),
                  ],
                ),
              // Rota çizgisi
              if (rideState.routePoints.isNotEmpty)
                PolylineLayer(
                  polylines: [
                    Polyline(
                      points: rideState.routePoints,
                      strokeWidth: 5,
                      color: Colors.blue,
                    ),
                  ],
                ),
              // Sürücü konumu marker'ı (pulse animasyonlu taksi üstten görünüş)
              if (rideState.driverLocation != null)
                MarkerLayer(
                  markers: [
                    Marker(
                      point: rideState.driverLocation!,
                      width: 56,
                      height: 56,
                      child: _PulsingMarker(heading: rideState.driverHeading),
                    ),
                  ],
                ),
              // Yakındaki müsait sürücü marker'ları
              if (rideState.rideStatus == 'idle' && rideState.nearbyDrivers.isNotEmpty)
                MarkerLayer(
                  markers: rideState.nearbyDrivers.map((d) => Marker(
                    point: LatLng(d.lat, d.lon),
                    width: 34,
                    height: 34,
                    child: Opacity(
                      opacity: d.isOnRide ? 0.4 : 1.0,
                      child: Image.asset('assets/images/taxi_top.png'),
                    ),
                  )).toList(),
                ),
            ],
          ),

          // Arama kutusu + Menü - yanyana, üstte
          Positioned(
            top: topPadding + 12,
            left: 16,
            right: 16,
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                Row(
                  children: [
                    Expanded(
                      child: CompositedTransformTarget(
                        link: _layerLink,
                        child: TextField(
                          controller: _destinationController,
                          readOnly: rideState.rideStatus != 'idle',
                          decoration: InputDecoration(
                            hintText: 'Nereye?',
                            prefixIcon: const Icon(Icons.search),
                            suffixIcon: rideState.rideStatus == 'idle' && _destinationController.text.isNotEmpty
                                ? IconButton(
                                    icon: const Icon(Icons.clear),
                                    onPressed: () {
                                      _destinationController.clear();
                                      _removeOverlay();
                                      ref.read(rideProvider.notifier).clearRoute();
                                      setState(() {});
                                    },
                                  )
                                : null,
                            border: OutlineInputBorder(
                              borderRadius: BorderRadius.circular(12),
                              borderSide: BorderSide.none,
                            ),
                            filled: true,
                            contentPadding: const EdgeInsets.symmetric(
                              horizontal: 16,
                              vertical: 12,
                            ),
                          ),
                          onChanged: (value) {
                            setState(() {});
                            _onSearchChanged(value);
                          },
                        ),
                      ),
                    ),
                    const SizedBox(width: 12),
                    Container(
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
                  ],
                ),
                // Konum butonu — haritayı kullanıcı konumuna ortalar
                Align(
                  alignment: Alignment.centerRight,
                  child: Padding(
                    padding: const EdgeInsets.only(top: 8),
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
                        icon: const Icon(Icons.my_location),
                        onPressed: _refreshLocation,
                      ),
                    ),
                  ),
                ),
              ],
            ),
          ),
          // Birleşik ride kartı — tüm durumlar için tek card
          if (rideState.routeInfo != null ||
              (rideState.rideStatus != 'idle' && rideState.rideStatus != 'cancelled'))
            Positioned(
              bottom: MediaQuery.of(context).padding.bottom,
              left: 0,
              right: 0,
              child: Container(
                margin: const EdgeInsets.all(16),
                padding: const EdgeInsets.fromLTRB(20, 20, 20, 32),
                decoration: BoxDecoration(
                  color: Colors.white,
                  borderRadius: BorderRadius.circular(20),
                  boxShadow: [
                    BoxShadow(
                      color: Colors.black.withAlpha(40),
                      blurRadius: 12,
                      offset: const Offset(0, -2),
                    ),
                  ],
                ),
                child: _buildRideStatusCard(rideState),
              ),
            ),
        ],
      ),
    );
  }

  Widget _buildRideStatusCard(RideState rideState) {
    switch (rideState.rideStatus) {
      case 'idle':
        if (rideState.routeInfo == null) return const SizedBox.shrink();
        return Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Row(
              children: [
                Icon(Icons.route, color: Colors.blue[700], size: 20),
                const SizedBox(width: 8),
                Text(
                  rideState.routeInfo!.formattedDistance,
                  style: const TextStyle(fontWeight: FontWeight.bold),
                ),
                const SizedBox(width: 4),
                Text(
                  '• ${rideState.routeInfo!.formattedDuration}',
                  style: TextStyle(color: Colors.grey[600]),
                ),
                const Spacer(),
                ElevatedButton(
                  onPressed: () async {
                    await ref.read(rideProvider.notifier).requestRide();
                  },
                  style: ElevatedButton.styleFrom(
                    backgroundColor: Colors.green,
                    foregroundColor: Colors.white,
                    disabledBackgroundColor: Colors.grey,
                    padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
                    shape: RoundedRectangleBorder(
                      borderRadius: BorderRadius.circular(8),
                    ),
                  ),
                  child: const Text('Taksi Çağır'),
                ),
              ],
            ),
            if (rideState.fareInfo != null) ...[
              const SizedBox(height: 10),
              const Divider(height: 1),
              const SizedBox(height: 10),
              _FareDetailRow(fareInfo: rideState.fareInfo!),
            ],
          ],
        );

      case 'searching':
        return Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Row(
              children: [
                const CircularProgressIndicator(strokeWidth: 2),
                const SizedBox(width: 16),
                const Expanded(
                  child: Text('Sürücü aranıyor...', style: TextStyle(fontSize: 16, fontWeight: FontWeight.bold)),
                ),
              ],
            ),
            if (rideState.routeInfo != null) ...[
              const SizedBox(height: 8),
              Row(
                children: [
                  Icon(Icons.route, color: Colors.blue[700], size: 18),
                  const SizedBox(width: 6),
                  Text(
                    rideState.routeInfo!.formattedDistance,
                    style: const TextStyle(fontWeight: FontWeight.bold, fontSize: 14),
                  ),
                  const SizedBox(width: 4),
                  Text(
                    '• ${rideState.routeInfo!.formattedDuration}',
                    style: TextStyle(color: Colors.grey[600], fontSize: 14),
                  ),
                ],
              ),
            ],
            if (rideState.fareInfo != null) ...[
              const SizedBox(height: 10),
              const Divider(height: 1),
              const SizedBox(height: 10),
              _FareDetailRow(fareInfo: rideState.fareInfo!),
            ],
            const SizedBox(height: 12),
            SizedBox(
              width: double.infinity,
              child: OutlinedButton.icon(
                onPressed: () => _cancelRide(),
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
          ],
        );

      case 'accepted':
        final driver = rideState.assignedDriver;
        String? etaText;
        if (rideState.driverEtaSeconds != null) {
          final totalMins = (rideState.driverEtaSeconds! / 60).ceil();
          if (totalMins >= 60) {
            final hrs = totalMins ~/ 60;
            final mins = totalMins % 60;
            etaText = '~$hrs S $mins Dk';
          } else {
            etaText = '~$totalMins Dk';
          }
        }
        return Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          mainAxisSize: MainAxisSize.min,
          children: [
            Row(
              children: [
                const Icon(Icons.local_taxi, color: Colors.amber, size: 28),
                const SizedBox(width: 12),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        driver?.fullName ?? 'Sürücü',
                        style: const TextStyle(fontSize: 16, fontWeight: FontWeight.bold),
                      ),
                      Text(
                        '${driver?.vehicleModel ?? ''} • ${driver?.vehiclePlate ?? ''}',
                        style: TextStyle(color: Colors.grey[600], fontSize: 13),
                      ),
                    ],
                  ),
                ),
                if (driver?.phone != null)
                  IconButton(
                    icon: const Icon(Icons.phone, color: Colors.green),
                    onPressed: () async {
                      final uri = Uri.parse('tel:${driver!.phone}');
                      if (await canLaunchUrl(uri)) launchUrl(uri);
                    },
                  ),
              ],
            ),
            const SizedBox(height: 4),
            Row(
              mainAxisAlignment: MainAxisAlignment.spaceBetween,
              children: [
                Row(
                  children: [
                    const Icon(Icons.directions_car, size: 16, color: Colors.green),
                    const SizedBox(width: 4),
                    const Text(
                      'Sürücü yolda',
                      style: TextStyle(color: Colors.green, fontWeight: FontWeight.w500),
                    ),
                    if (etaText != null) ...[
                      const SizedBox(width: 4),
                      Text(
                        etaText,
                        style: TextStyle(color: Colors.grey[600], fontSize: 13, fontWeight: FontWeight.w500),
                      ),
                    ],
                  ],
                ),
                if (rideState.routeInfo != null)
                  Row(
                    children: [
                      Icon(Icons.route, color: Colors.blue[700], size: 16),
                      const SizedBox(width: 4),
                      Text(
                        rideState.routeInfo!.formattedDistance,
                        style: const TextStyle(fontWeight: FontWeight.bold, fontSize: 14),
                      ),
                      const SizedBox(width: 4),
                      Text(
                        rideState.routeInfo!.formattedDuration,
                        style: TextStyle(color: Colors.grey[600], fontSize: 14),
                      ),
                    ],
                  ),
              ],
            ),
            if (rideState.fareInfo != null) ...[
              const SizedBox(height: 10),
              const Divider(height: 1),
              const SizedBox(height: 10),
              _FareDetailRow(fareInfo: rideState.fareInfo!),
            ],
            const SizedBox(height: 12),
            SizedBox(
              width: double.infinity,
              child: OutlinedButton.icon(
                onPressed: () => _cancelRide(),
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
          ],
        );

      case 'picked_up':
        final driver = rideState.assignedDriver;
        return Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          mainAxisSize: MainAxisSize.min,
          children: [
            Row(
              children: [
                const Icon(Icons.local_taxi, color: Colors.amber, size: 28),
                const SizedBox(width: 12),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        driver?.fullName ?? 'Sürücü',
                        style: const TextStyle(fontSize: 16, fontWeight: FontWeight.bold),
                      ),
                      Text(
                        '${driver?.vehicleModel ?? ''} • ${driver?.vehiclePlate ?? ''}',
                        style: TextStyle(color: Colors.grey[600], fontSize: 13),
                      ),
                    ],
                  ),
                ),
                if (driver?.phone != null)
                  IconButton(
                    icon: const Icon(Icons.phone, color: Colors.green),
                    onPressed: () async {
                      final uri = Uri.parse('tel:${driver!.phone}');
                      if (await canLaunchUrl(uri)) launchUrl(uri);
                    },
                  ),
              ],
            ),
            const SizedBox(height: 4),
            Row(
              mainAxisAlignment: MainAxisAlignment.spaceBetween,
              children: [
                Row(
                  children: [
                    const Icon(Icons.check_circle, size: 16, color: Colors.green),
                    const SizedBox(width: 4),
                    const Text(
                      'Yolculuk başladı',
                      style: TextStyle(color: Colors.green, fontWeight: FontWeight.w500),
                    ),
                  ],
                ),
                if (rideState.routeInfo != null)
                  Row(
                    children: [
                      Text(
                        rideState.routeInfo!.formattedDistance,
                        style: const TextStyle(fontWeight: FontWeight.bold, fontSize: 14),
                      ),
                      const SizedBox(width: 4),
                      Text(
                        '• ${rideState.routeInfo!.formattedDuration}',
                        style: TextStyle(color: Colors.grey[600], fontSize: 14),
                      ),
                    ],
                  ),
              ],
            ),
            if (rideState.fareInfo != null) ...[
              const SizedBox(height: 10),
              const Divider(height: 1),
              const SizedBox(height: 10),
              _FareDetailRow(fareInfo: rideState.fareInfo!),
            ],
            // İptal/Tamamla butonları kaldırıldı — yolculuk başladıktan sonra
            // sadece sürücü iptal/tamamlayabilir
            // const SizedBox(height: 12),
            // Row(
            //   children: [
            //     Expanded(
            //       child: OutlinedButton.icon(
            //         onPressed: () => _cancelRide(),
            //         icon: const Icon(Icons.cancel_outlined, size: 18),
            //         label: const Text('İptal'),
            //         style: OutlinedButton.styleFrom(
            //           foregroundColor: Colors.red,
            //           side: const BorderSide(color: Colors.red),
            //           padding: const EdgeInsets.symmetric(vertical: 14),
            //           shape: RoundedRectangleBorder(
            //             borderRadius: BorderRadius.circular(10),
            //           ),
            //         ),
            //       ),
            //     ),
            //     const SizedBox(width: 12),
            //     Expanded(
            //       flex: 2,
            //       child: ElevatedButton.icon(
            //         onPressed: () => ref.read(rideProvider.notifier).completeRide(),
            //         icon: const Icon(Icons.check_circle, size: 18),
            //         label: const Text('Tamamla'),
            //         style: ElevatedButton.styleFrom(
            //           backgroundColor: Colors.green,
            //           foregroundColor: Colors.white,
            //           padding: const EdgeInsets.symmetric(vertical: 14),
            //           shape: RoundedRectangleBorder(
            //             borderRadius: BorderRadius.circular(10),
            //           ),
            //         ),
            //       ),
            //     ),
            //   ],
            // ),
          ],
        );

      case 'completed':
        final completedFareInfo = rideState.fareInfo;
        return Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Row(
              children: [
                const Icon(Icons.check_circle, color: Colors.green, size: 28),
                const SizedBox(width: 12),
                const Expanded(
                  child: Text(
                    'Yolculuk tamamlandı',
                    style: TextStyle(fontWeight: FontWeight.bold, fontSize: 16),
                  ),
                ),
                TextButton(
                  onPressed: () {
                    _destinationController.clear();
                    _removeOverlay();
                    ref.read(rideProvider.notifier).resetRide();
                  },
                  child: const Text('Kapat'),
                ),
              ],
            ),
            if (completedFareInfo != null) ...[
              const SizedBox(height: 10),
              const Divider(height: 1),
              const SizedBox(height: 10),
              _FareDetailRow(fareInfo: completedFareInfo),
            ],
            const SizedBox(height: 8),
            const Text(
              'Ödemeyi sürücüye nakit veya kart ile yapabilirsiniz.',
              style: TextStyle(color: Colors.grey, fontSize: 13),
            ),
          ],
        );

      default:
        return const SizedBox.shrink();
    }
  }
}

/// Ücret detayı satırı — açılış ücreti, km başına ücret, tahmini tutar.
///
/// Backend'den gelen FareInfo'yu gösterir. Yolcu ekranında:
/// - Rota kartında (taksi çağırmadan önce)
/// - searching/accepted/picked_up/completed kartlarında
class _FareDetailRow extends StatelessWidget {
  final FareInfo fareInfo;
  const _FareDetailRow({required this.fareInfo});

  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        // Ücret kalemleri — 3 sütun
        Row(
          children: [
            _FareChip(
              icon: Icons.flag_outlined,
              label: 'Açılış',
              value: fareInfo.formattedOpeningFee,
              color: Colors.blue,
            ),
            const SizedBox(width: 8),
            _FareChip(
              icon: Icons.speed_outlined,
              label: 'Km başına',
              value: fareInfo.formattedPerKm,
              color: Colors.orange,
            ),
            const SizedBox(width: 8),
            _FareChip(
              icon: Icons.payments_outlined,
              label: 'Min. ücret',
              value: fareInfo.formattedMinFare,
              color: Colors.purple,
            ),
          ],
        ),
        const SizedBox(height: 10),
        // Tahmini toplam
        Container(
          width: double.infinity,
          padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
          decoration: BoxDecoration(
            color: Colors.green.withAlpha(18),
            borderRadius: BorderRadius.circular(8),
          ),
          child: Row(
            mainAxisAlignment: MainAxisAlignment.spaceBetween,
            children: [
              const Text(
                'Tahmini ücret',
                style: TextStyle(fontSize: 13, color: Colors.grey),
              ),
              Text(
                fareInfo.formattedEstimate,
                style: const TextStyle(
                  fontSize: 16,
                  fontWeight: FontWeight.bold,
                  color: Colors.green,
                ),
              ),
            ],
          ),
        ),
      ],
    );
  }
}

/// Tek ücret kalemi chip'i
class _FareChip extends StatelessWidget {
  final IconData icon;
  final String label;
  final String value;
  final Color color;

  const _FareChip({
    required this.icon,
    required this.label,
    required this.value,
    required this.color,
  });

  @override
  Widget build(BuildContext context) {
    return Expanded(
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 6),
        decoration: BoxDecoration(
          color: color.withAlpha(15),
          borderRadius: BorderRadius.circular(8),
          border: Border.all(color: color.withAlpha(40)),
        ),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                Icon(icon, size: 12, color: color),
                const SizedBox(width: 4),
                Text(label,
                    style: TextStyle(fontSize: 10, color: color, fontWeight: FontWeight.w500)),
              ],
            ),
            const SizedBox(height: 2),
            Text(
              value,
              style: const TextStyle(fontSize: 13, fontWeight: FontWeight.bold),
            ),
          ],
        ),
      ),
    );
  }
}

/// Haritada sürücü konumunu gösteren animasyonlu marker widget'ı.
///
/// Yeşil daire üzerine beyaz taksi ikonu yerleştirir.
/// Dış halka 1200ms'de büyüyerek soluklaşan bir pulse efekti oluşturur.
/// Backend'den gelen driver_location WS mesajı ile konumu güncellenir
/// (DriverLocationMessage → rideProvider.driverLocation).
class _PulsingMarker extends StatefulWidget {
  final double? heading;
  const _PulsingMarker({this.heading});

  @override
  State<_PulsingMarker> createState() => _PulsingMarkerState();
}

/// Sürücü konumu marker'ı — pulse animasyonlu taksi üstten görünüş.
///
/// Pulse animasyonu: dış halka 0.4→1.0 ölçek, opaklık 0.8→0.0 (1200ms döngü).
/// Merkez: taksi üstten görünüş fotoğrafı.
class _PulsingMarkerState extends State<_PulsingMarker>
    with SingleTickerProviderStateMixin {
  late final AnimationController _ctrl;
  late final Animation<double> _scale;
  late final Animation<double> _opacity;

  @override
  void initState() {
    super.initState();
    _ctrl = AnimationController(
      vsync: this,
      duration: const Duration(milliseconds: 1200),
    )..repeat();
    _scale = Tween<double>(begin: 0.4, end: 1.0).animate(
      CurvedAnimation(parent: _ctrl, curve: Curves.easeOut),
    );
    _opacity = Tween<double>(begin: 0.8, end: 0.0).animate(
      CurvedAnimation(parent: _ctrl, curve: Curves.easeOut),
    );
  }

  @override
  void dispose() {
    _ctrl.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    // Taksi üstten görünüş resmi varsayılan olarak sola (batı, 270°) bakıyor.
    // heading 0°=Kuzey, 90°=Doğu, 180°=Güney, 270°=Batı.
    // Saat yönünde dönüş: (heading + 90)° — sol↔kuzey arası 90° fark.
    final angle = (widget.heading != null)
        ? (widget.heading! + 90) * pi / 180
        : 0.0;
    return Stack(
      alignment: Alignment.center,
      children: [
        // Dalgalanan halka
        AnimatedBuilder(
          animation: _ctrl,
          builder: (_, _) => Transform.scale(
            scale: _scale.value,
            child: Container(
              width: 56,
              height: 56,
              decoration: BoxDecoration(
                shape: BoxShape.circle,
                color: Colors.green.withAlpha((_opacity.value * 255).toInt()),
              ),
            ),
          ),
        ),
        // Taksi üstten görünüş — yöne göre döndürülmüş
        Transform.rotate(
          angle: angle,
          child: Image.asset(
'assets/images/taxi_top.png',
              width: 38,
              height: 38,
          ),
        ),
      ],
    );
  }
}