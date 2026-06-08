class UserProfile {
  final String? bio;
  final String? phone;
  final String? website;
  final String? location;

  UserProfile({
    this.bio,
    this.phone,
    this.website,
    this.location,
  });

  factory UserProfile.fromJson(Map<String, dynamic> json) {
    return UserProfile(
      bio: json['bio'] as String?,
      phone: json['phone'] as String?,
      website: json['website'] as String?,
      location: json['location'] as String?,
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'bio': bio,
      'phone': phone,
      'website': website,
      'location': location,
    };
  }
}

/// Kullanıcı modeli.
///
/// Backend GET /api/auth/me veya login/register yanıtlarından deserialize edilir.
/// userType alanı sürücü modu routing'inde kullanılır: 'driver' ise DriverHomePage'e yönlendirilir.
class User {
  final int id;
  final String username;
  final String email;
  final String? firstName;
  final String? lastName;
  final String? birthDate;
  final UserProfile? profile;
  final DateTime? createdAt;
  final String userType; // 'B2C', 'B2B', 'driver'

  User({
    required this.id,
    required this.username,
    required this.email,
    this.firstName,
    this.lastName,
    this.birthDate,
    this.profile,
    this.createdAt,
    this.userType = 'B2C',
  });

  factory User.fromJson(Map<String, dynamic> json) {
    return User(
      id: json['id'] as int,
      username: json['username'] as String,
      email: json['email'] as String,
      firstName: json['first_name'] as String?,
      lastName: json['last_name'] as String?,
      birthDate: json['birth_date'] as String?,
      profile: json['profile'] != null
          ? UserProfile.fromJson(json['profile'] as Map<String, dynamic>)
          : null,
      createdAt: json['created_at'] != null
          ? DateTime.tryParse(json['created_at'] as String)
          : null,
      userType: json['user_type'] as String? ?? 'B2C',
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'id': id,
      'username': username,
      'email': email,
      'first_name': firstName,
      'last_name': lastName,
      'birth_date': birthDate,
      'profile': profile?.toJson(),
      'created_at': createdAt?.toIso8601String(),
      'user_type': userType,
    };
  }

  String get fullName {
    if (firstName != null && lastName != null) {
      return '$firstName $lastName';
    }
    return username;
  }

  String get displayName => fullName;

  bool get isDriver => userType == 'driver';

  int? get age {
    if (birthDate == null) return null;
    try {
      final birth = DateTime.parse(birthDate!);
      final now = DateTime.now();
      int age = now.year - birth.year;
      if (now.month < birth.month || 
          (now.month == birth.month && now.day < birth.day)) {
        age--;
      }
      return age;
    } catch (e) {
      return null;
    }
  }
}