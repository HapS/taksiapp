import 'package:go_router/go_router.dart';
import 'login_page.dart';
import 'register_page.dart';
import 'profile_page.dart';

class AuthRoutes {
  static List<GoRoute> routes = [
    GoRoute(
      path: '/login',
      name: 'login',
      builder: (context, state) => const LoginPage(),
    ),
    GoRoute(
      path: '/register',
      name: 'register',
      builder: (context, state) => const RegisterPage(),
    ),
    GoRoute(
      path: '/profile',
      name: 'profile',
      builder: (context, state) => const ProfilePage(),
    ),
  ];
}
