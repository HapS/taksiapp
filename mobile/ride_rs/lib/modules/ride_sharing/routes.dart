import 'package:go_router/go_router.dart';
import 'home_page.dart';
import 'ride_history_page.dart';

/// Ride sharing modülü route tanımları.
///
/// /rideHome path'ini RideHomePage'e bağlar.
/// Ana uygulama router'ı (app_router.dart) bu route'ları dahil eder.
class RideRoutes {
  static List<GoRoute> routes = [
    GoRoute(
      path: '/rideHome',
      name: 'rideHome',
      builder: (context, state) => const RideHomePage(),
    ),
    GoRoute(
      path: '/rideHistory',
      name: 'rideHistory',
      builder: (context, state) => const RideHistoryPage(),
    ),
  ];
}