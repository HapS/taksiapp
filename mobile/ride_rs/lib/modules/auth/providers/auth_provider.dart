import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import '../models/user_model.dart';
import '../services/auth_service.dart';

// Auth Service Provider
final authServiceProvider = Provider<AuthService>((ref) {
  return AuthService();
});

// Auth State
enum AuthStatus { initial, loading, authenticated, unauthenticated, error }

class AuthState {
  final AuthStatus status;
  final User? user;
  final String? error;

  const AuthState({this.status = AuthStatus.initial, this.user, this.error});

  AuthState copyWith({AuthStatus? status, User? user, String? error}) {
    return AuthState(
      status: status ?? this.status,
      user: user ?? this.user,
      error: error,
    );
  }

  bool get isAuthenticated => status == AuthStatus.authenticated;
  bool get isLoading => status == AuthStatus.loading;
}

// Auth Notifier
class AuthNotifier extends Notifier<AuthState> {
  late AuthService _authService;

  @override
  AuthState build() {
    _authService = ref.watch(authServiceProvider);
    return const AuthState();
  }

  Future<void> checkAuthStatus() async {
    state = state.copyWith(status: AuthStatus.loading);

    try {
      final hasTokens = await _authService.hasValidTokens();
      if (!hasTokens) {
        state = const AuthState(status: AuthStatus.unauthenticated);
        return;
      }

      final isValidToken = await _authService.ensureValidToken();
      if (!isValidToken) {
        await _authService.clearTokens();
        state = const AuthState(status: AuthStatus.unauthenticated);
        return;
      }

      final profileResponse = await _authService.getProfile();
      if (profileResponse.success && profileResponse.user != null) {
        debugPrint('AuthNotifier: user_type = ${profileResponse.user!.userType}, isDriver = ${profileResponse.user!.isDriver}');
        state = AuthState(
          status: AuthStatus.authenticated,
          user: profileResponse.user,
        );
      } else {
        debugPrint('AuthNotifier: profile fetch failed: ${profileResponse.error}');
        await _authService.clearTokens();
        state = const AuthState(status: AuthStatus.unauthenticated);
      }
    } catch (e) {
      debugPrint('AuthNotifier: checkAuthStatus error: $e');
      await _authService.clearTokens();
      state = const AuthState(status: AuthStatus.unauthenticated);
    }
  }

  Future<bool> login(String username, String password) async {
    state = state.copyWith(status: AuthStatus.loading);

    final response = await _authService.login(username, password);

    if (response.success && response.tokens != null) {
      if (response.user != null) {
        debugPrint('AuthNotifier: login user_type = ${response.user!.userType}, isDriver = ${response.user!.isDriver}');
        state = AuthState(
          status: AuthStatus.authenticated,
          user: response.user,
        );
        return true;
      }
      final profileResponse = await _authService.getProfile();
      if (profileResponse.success && profileResponse.user != null) {
        debugPrint('AuthNotifier: login profile user_type = ${profileResponse.user!.userType}, isDriver = ${profileResponse.user!.isDriver}');
        state = AuthState(
          status: AuthStatus.authenticated,
          user: profileResponse.user,
        );
        return true;
      }
    }

    state = AuthState(
      status: AuthStatus.error,
      error: response.message ?? response.error ?? 'Giriş başarısız',
    );
    return false;
  }

  Future<bool> register(String username, String password, String email) async {
    state = state.copyWith(status: AuthStatus.loading);

    final response = await _authService.register(username, password, email);

    if (response.success && response.tokens != null) {
      if (response.user != null) {
        state = AuthState(
          status: AuthStatus.authenticated,
          user: response.user,
        );
        return true;
      }
      final profileResponse = await _authService.getProfile();
      if (profileResponse.success && profileResponse.user != null) {
        state = AuthState(
          status: AuthStatus.authenticated,
          user: profileResponse.user,
        );
        return true;
      }
    }

    state = AuthState(
      status: AuthStatus.error,
      error: response.message ?? response.error ?? 'Kayıt başarısız',
    );
    return false;
  }

  Future<void> logout() async {
    await _authService.logout();
    state = const AuthState(status: AuthStatus.unauthenticated);
  }

  Future<void> refreshProfile() async {
    final profileResponse = await _authService.getProfile();
    if (profileResponse.success && profileResponse.user != null) {
      state = state.copyWith(user: profileResponse.user);
    }
  }

  void clearError() {
    state = state.copyWith(error: null);
  }
}

// Auth Provider
final authProvider = NotifierProvider<AuthNotifier, AuthState>(() {
  return AuthNotifier();
});

// Profile Provider
final profileProvider = FutureProvider<User?>((ref) async {
  final authService = ref.watch(authServiceProvider);
  final response = await authService.getProfile();
  return response.user;
});
