import 'user_model.dart';

class AuthTokens {
  final String accessToken;
  final String refreshToken;

  AuthTokens({required this.accessToken, required this.refreshToken});

  factory AuthTokens.fromJson(Map<String, dynamic> json) {
    return AuthTokens(
      accessToken: json['access_token'] as String,
      refreshToken: json['refresh_token'] as String,
    );
  }

  Map<String, dynamic> toJson() {
    return {'access_token': accessToken, 'refresh_token': refreshToken};
  }
}

class AuthResponse {
  final bool success;
  final AuthTokens? tokens;
  final User? user;
  final String? error;
  final String? message;

  AuthResponse({
    required this.success,
    this.tokens,
    this.user,
    this.error,
    this.message,
  });

  factory AuthResponse.fromJson(Map<String, dynamic> json) {
    return AuthResponse(
      success: json['success'] as bool,
      tokens: json['tokens'] != null
          ? AuthTokens.fromJson(json['tokens'] as Map<String, dynamic>)
          : null,
      user: json['user'] != null
          ? User.fromJson(json['user'] as Map<String, dynamic>)
          : null,
      error: json['error'] as String?,
      message: json['message'] as String?,
    );
  }
}

class ProfileResponse {
  final bool success;
  final User? user;
  final String? error;

  ProfileResponse({required this.success, this.user, this.error});

  factory ProfileResponse.fromJson(Map<String, dynamic> json) {
    return ProfileResponse(
      success: json['success'] as bool,
      user: json['data'] != null
          ? User.fromJson(json['data'] as Map<String, dynamic>)
          : null,
      error: json['error'] as String?,
    );
  }
}

class RefreshResponse {
  final bool success;
  final String? accessToken;
  final String? refreshToken;
  final String? error;

  RefreshResponse({
    required this.success,
    this.accessToken,
    this.refreshToken,
    this.error,
  });

  factory RefreshResponse.fromJson(Map<String, dynamic> json) {
    final data = json['data'] as Map<String, dynamic>?;
    return RefreshResponse(
      success: json['success'] as bool,
      accessToken: data?['access_token'] as String?,
      refreshToken: data?['refresh_token'] as String?,
      error: json['error'] as String?,
    );
  }
}
