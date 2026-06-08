import 'dart:convert';
import 'package:flutter/foundation.dart';
import 'package:http/http.dart' as http;
import 'package:flutter_secure_storage/flutter_secure_storage.dart';
import '../../../core/config/app_config.dart';
import '../models/auth_response.dart';

class AuthService {
  static const String baseUrl = '${AppConfig.apiEndpoint}/auth';
  static const _storage = FlutterSecureStorage();

  static const String _accessTokenKey = 'access_token';
  static const String _refreshTokenKey = 'refresh_token';

  // Token storage methods
  Future<void> saveTokens(String accessToken, String refreshToken) async {
    await _storage.write(key: _accessTokenKey, value: accessToken);
    await _storage.write(key: _refreshTokenKey, value: refreshToken);
  }

  Future<String?> getAccessToken() async {
    return await _storage.read(key: _accessTokenKey);
  }

  Future<String?> getRefreshToken() async {
    return await _storage.read(key: _refreshTokenKey);
  }

  Future<void> clearTokens() async {
    await _storage.delete(key: _accessTokenKey);
    await _storage.delete(key: _refreshTokenKey);
  }

  Future<bool> hasValidTokens() async {
    final accessToken = await getAccessToken();
    return accessToken != null && accessToken.isNotEmpty;
  }

  // JWT token'ın expire olup olmadığını kontrol et
  bool isTokenExpired(String token) {
    try {
      final parts = token.split('.');
      if (parts.length != 3) return true;

      final payload = parts[1];
      final normalized = base64Url.normalize(payload);
      final decoded = utf8.decode(base64Url.decode(normalized));
      final payloadMap = json.decode(decoded) as Map<String, dynamic>;

      if (payloadMap['exp'] != null) {
        final exp = payloadMap['exp'] as int;
        final now = DateTime.now().millisecondsSinceEpoch ~/ 1000;
        return now >= exp;
      }
      return true;
    } catch (e) {
      return true;
    }
  }

  // Token'ı kontrol et ve gerekirse yenile
  Future<bool> ensureValidToken() async {
    final accessToken = await getAccessToken();
    if (accessToken == null) return false;

    if (isTokenExpired(accessToken)) {
      final refreshResult = await refreshTokens();
      return refreshResult.success;
    }
    return true;
  }

  // Auth API methods
  Future<AuthResponse> login(String username, String password) async {
    try {
      final response = await http.post(
        Uri.parse('$baseUrl/login'),
        headers: {
          'Content-Type': 'application/json',
          'Cache-Control': 'no-cache',
        },
        body: jsonEncode({'username': username, 'password': password}),
      );

      final json = jsonDecode(response.body) as Map<String, dynamic>;
      final authResponse = AuthResponse.fromJson(json);

      if (authResponse.success && authResponse.tokens != null) {
        await saveTokens(
          authResponse.tokens!.accessToken,
          authResponse.tokens!.refreshToken,
        );
        debugPrint('Login successful: ${authResponse.user?.username}');
      }

      return authResponse;
    } catch (e) {
      debugPrint('Login error: ${e.toString()}');
      return AuthResponse(
        success: false,
        error: 'Bağlantı hatası: ${e.toString()}',
      );
    }
  }

  Future<AuthResponse> register(
    String username,
    String password,
    String email,
  ) async {
    try {
      final response = await http.post(
        Uri.parse('$baseUrl/register'),
        headers: {
          'Content-Type': 'application/json',
          'Cache-Control': 'no-cache',
        },
        body: jsonEncode({
          'username': username,
          'password': password,
          'email': email,
        }),
      );

      final json = jsonDecode(response.body) as Map<String, dynamic>;
      final authResponse = AuthResponse.fromJson(json);

      if (authResponse.success && authResponse.tokens != null) {
        await saveTokens(
          authResponse.tokens!.accessToken,
          authResponse.tokens!.refreshToken,
        );
      }

      return authResponse;
    } catch (e) {
      return AuthResponse(
        success: false,
        error: 'Bağlantı hatası: ${e.toString()}',
      );
    }
  }

  Future<RefreshResponse> refreshTokens() async {
    try {
      final refreshToken = await getRefreshToken();
      if (refreshToken == null) {
        return RefreshResponse(
          success: false,
          error: 'Refresh token bulunamadı',
        );
      }

      final response = await http.post(
        Uri.parse('$baseUrl/refresh'),
        headers: {
          'Content-Type': 'application/json',
          'Cache-Control': 'no-cache',
          'Authorization': 'Bearer $refreshToken',
        },
      );

      final json = jsonDecode(response.body) as Map<String, dynamic>;
      final refreshResponse = RefreshResponse.fromJson(json);

      if (refreshResponse.success && refreshResponse.accessToken != null) {
        await saveTokens(
          refreshResponse.accessToken!,
          refreshResponse.refreshToken ?? refreshToken,
        );
      }

      return refreshResponse;
    } catch (e) {
      return RefreshResponse(
        success: false,
        error: 'Bağlantı hatası: ${e.toString()}',
      );
    }
  }

  Future<ProfileResponse> getProfile() async {
    try {
      final accessToken = await getAccessToken();
      if (accessToken == null) {
        return ProfileResponse(
          success: false,
          error: 'Access token bulunamadı',
        );
      }

      final response = await http.get(
        Uri.parse('${AppConfig.apiEndpoint}/user/profile'),
        headers: {
          'Content-Type': 'application/json',
          'Cache-Control': 'no-cache',
          'Authorization': 'Bearer $accessToken',
        },
      );

      if (response.statusCode == 401) {
        final refreshResult = await refreshTokens();
        if (refreshResult.success) {
          return getProfile();
        } else {
          await clearTokens();
          return ProfileResponse(
            success: false,
            error: 'Oturum süresi doldu, lütfen tekrar giriş yapın',
          );
        }
      }

      final json = jsonDecode(response.body) as Map<String, dynamic>;
      return ProfileResponse.fromJson(json);
    } catch (e) {
      return ProfileResponse(
        success: false,
        error: 'Bağlantı hatası: ${e.toString()}',
      );
    }
  }

  Future<void> logout() async {
    await clearTokens();
  }
}
