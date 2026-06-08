import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'modules/auth/providers/auth_provider.dart';
import 'modules/auth/routes.dart';
import 'modules/settings/settings_page.dart';
import 'modules/ride_sharing/routes.dart';
import 'modules/ride_sharing/driver_home_page.dart';
import 'home_page.dart';

// Auth state notifier for router refresh
class AuthStateNotifier extends ChangeNotifier {
  AuthStatus _status = AuthStatus.initial;

  AuthStatus get status => _status;

  void update(AuthStatus newStatus) {
    if (_status != newStatus) {
      _status = newStatus;
      notifyListeners();
    }
  }
}

final authStateNotifierProvider = Provider<AuthStateNotifier>((ref) {
  final notifier = AuthStateNotifier();

  ref.listen(authProvider, (previous, next) {
    notifier.update(next.status);
  });

  return notifier;
});

// Router Provider
final routerProvider = Provider<GoRouter>((ref) {
  final authStateNotifier = ref.watch(authStateNotifierProvider);

  return GoRouter(
    initialLocation: '/',
    debugLogDiagnostics: true,
    refreshListenable: authStateNotifier,
    redirect: (context, state) {
      final authState = ref.read(authProvider);
      final isAuthenticated = authState.isAuthenticated;
      final isLoading =
          authState.status == AuthStatus.initial ||
          authState.status == AuthStatus.loading;
      final isAuthRoute =
          state.matchedLocation == '/login' ||
          state.matchedLocation == '/register';
      final isProtectedRoute = state.matchedLocation == '/profile';
      final isHistoryRoute = state.matchedLocation == '/rideHistory';
      final isDriverHome = state.matchedLocation == '/driver';
      final isPassengerHome = state.matchedLocation == '/';

      if (isLoading) {
        return null;
      }

      if (!isAuthenticated && (isProtectedRoute || isHistoryRoute || isDriverHome)) {
        return '/login';
      }

      if (isAuthenticated && isAuthRoute) {
        if (authState.user?.isDriver == true) {
          return '/driver';
        }
        return '/';
      }

      if (isAuthenticated && isPassengerHome && authState.user?.isDriver == true) {
        return '/driver';
      }

      if (isAuthenticated && isDriverHome && authState.user?.isDriver != true) {
        return '/';
      }

      return null;
    },
    routes: [
      GoRoute(
        path: '/',
        name: 'home',
        builder: (context, state) => const HomePage(),
      ),
      GoRoute(
        path: '/driver',
        name: 'driver_home',
        builder: (context, state) => const DriverHomePage(),
      ),
      ...RideRoutes.routes,
      // Auth routes (login, register, profile)
      ...AuthRoutes.routes,
      // Settings route
      GoRoute(
        path: '/settings',
        name: 'settings',
        builder: (context, state) => const SettingsPage(),
      ),
    ],
    errorBuilder: (context, state) => Scaffold(
      body: Center(
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            const Icon(Icons.error_outline, size: 64, color: Colors.red),
            const SizedBox(height: 16),
            Text(
              'Sayfa bulunamadı',
              style: Theme.of(context).textTheme.headlineSmall,
            ),
            const SizedBox(height: 8),
            Text(
              state.matchedLocation,
              style: Theme.of(
                context,
              ).textTheme.bodyMedium?.copyWith(color: Colors.grey[600]),
            ),
            const SizedBox(height: 24),
            ElevatedButton(
              onPressed: () => context.go('/'),
              child: const Text('Ana Sayfa'),
            ),
          ],
        ),
      ),
    ),
  );
});
