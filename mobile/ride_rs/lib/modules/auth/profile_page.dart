import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'providers/auth_provider.dart';

class ProfilePage extends ConsumerWidget {
  const ProfilePage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final authState = ref.watch(authProvider);
    final user = authState.user;

    if (user == null) {
      return Scaffold(
        appBar: AppBar(
          leading: IconButton(
            icon: const Icon(Icons.arrow_back),
            onPressed: () => context.go('/'),
          ),
          title: const Text('Profil'),
        ),
        body: const Center(child: Text('Kullanıcı bilgisi bulunamadı')),
      );
    }

    return Scaffold(
      appBar: AppBar(
        backgroundColor: Theme.of(context).colorScheme.inversePrimary,
        leading: IconButton(
          icon: const Icon(Icons.arrow_back),
          onPressed: () => context.go('/'),
        ),
        title: const Text('Profil'),
        actions: [
          IconButton(
            icon: const Icon(Icons.refresh),
            onPressed: () async {
              await ref.read(authProvider.notifier).refreshProfile();
              if (context.mounted) {
                ScaffoldMessenger.of(context).showSnackBar(
                  const SnackBar(content: Text('Profil güncellendi')),
                );
              }
            },
            tooltip: 'Yenile',
          ),
        ],
      ),
      body: SafeArea(
        top: false,
        child: SingleChildScrollView(
          padding: const EdgeInsets.all(16.0),
          child: Column(
          children: [
            Card(
              child: Padding(
                padding: const EdgeInsets.all(24.0),
                child: Column(
                  children: [
                    CircleAvatar(
                      radius: 50,
                      backgroundColor: Theme.of(
                        context,
                      ).colorScheme.primaryContainer,
                      child: Text(
                        user.username.substring(0, 1).toUpperCase(),
                        style: TextStyle(
                          fontSize: 36,
                          fontWeight: FontWeight.bold,
                          color: Theme.of(context).colorScheme.primary,
                        ),
                      ),
                    ),
                    const SizedBox(height: 16),
                    Text(
                      user.fullName,
                      style: Theme.of(context).textTheme.headlineSmall
                          ?.copyWith(fontWeight: FontWeight.bold),
                    ),
                    const SizedBox(height: 4),
                    Text(
                      '@${user.username}',
                      style: Theme.of(
                        context,
                      ).textTheme.bodyLarge?.copyWith(color: Colors.grey[600]),
                    ),
                    if (user.profile?.bio != null) ...[
                      const SizedBox(height: 12),
                      Container(
                        width: double.infinity,
                        padding: const EdgeInsets.all(16),
                        decoration: BoxDecoration(
                          color: Theme.of(
                            context,
                          ).colorScheme.surfaceVariant.withOpacity(0.3),
                          borderRadius: BorderRadius.circular(12),
                          border: Border.all(
                            color: Theme.of(
                              context,
                            ).colorScheme.outline.withOpacity(0.2),
                          ),
                        ),
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            Row(
                              children: [
                                Icon(
                                  Icons.info_outline,
                                  size: 16,
                                  color: Theme.of(context).colorScheme.primary,
                                ),
                                const SizedBox(width: 8),
                                Text(
                                  'Hakkımda',
                                  style: TextStyle(
                                    fontSize: 12,
                                    fontWeight: FontWeight.w500,
                                    color: Theme.of(
                                      context,
                                    ).colorScheme.primary,
                                  ),
                                ),
                              ],
                            ),
                            const SizedBox(height: 8),
                            Text(
                              user.profile!.bio!,
                              style: Theme.of(context).textTheme.bodyMedium,
                            ),
                          ],
                        ),
                      ),
                    ],
                  ],
                ),
              ),
            ),
            const SizedBox(height: 16),
            Card(
              child: Column(
                children: [
                  _buildProfileItem(
                    context,
                    Icons.person_outline,
                    'Kullanıcı Adı',
                    user.username,
                  ),
                  const Divider(height: 1),
                  _buildProfileItem(
                    context,
                    Icons.email_outlined,
                    'E-posta',
                    user.email,
                  ),
                  if (user.firstName != null) ...[
                    const Divider(height: 1),
                    _buildProfileItem(
                      context,
                      Icons.badge_outlined,
                      'Ad',
                      user.firstName!,
                    ),
                  ],
                  if (user.lastName != null) ...[
                    const Divider(height: 1),
                    _buildProfileItem(
                      context,
                      Icons.badge_outlined,
                      'Soyad',
                      user.lastName!,
                    ),
                  ],
                  if (user.birthDate != null) ...[
                    const Divider(height: 1),
                    _buildProfileItem(
                      context,
                      Icons.cake_outlined,
                      'Doğum Tarihi',
                      _formatBirthDate(user.birthDate!),
                    ),
                  ],
                  if (user.age != null) ...[
                    const Divider(height: 1),
                    _buildProfileItem(
                      context,
                      Icons.person_outline,
                      'Yaş',
                      '${user.age} yaşında',
                    ),
                  ],
                  if (user.profile?.phone != null) ...[
                    const Divider(height: 1),
                    _buildProfileItem(
                      context,
                      Icons.phone_outlined,
                      'Telefon',
                      user.profile!.phone!,
                    ),
                  ],
                  if (user.profile?.location != null) ...[
                    const Divider(height: 1),
                    _buildProfileItem(
                      context,
                      Icons.location_on_outlined,
                      'Konum',
                      user.profile!.location!,
                    ),
                  ],
                  if (user.profile?.website != null) ...[
                    const Divider(height: 1),
                    _buildProfileItem(
                      context,
                      Icons.language_outlined,
                      'Website',
                      user.profile!.website!,
                    ),
                  ],
                  if (user.createdAt != null) ...[
                    const Divider(height: 1),
                    _buildProfileItem(
                      context,
                      Icons.calendar_today_outlined,
                      'Kayıt Tarihi',
                      _formatDate(user.createdAt!),
                    ),
                  ],
                ],
              ),
            ),
            const SizedBox(height: 32),
            SizedBox(
              width: double.infinity,
              child: OutlinedButton.icon(
                onPressed: () async {
                  final confirm = await showDialog<bool>(
                    context: context,
                    builder: (context) => AlertDialog(
                      title: const Text('Çıkış Yap'),
                      content: const Text(
                        'Çıkış yapmak istediğinizden emin misiniz?',
                      ),
                      actions: [
                        TextButton(
                          onPressed: () => Navigator.of(context).pop(false),
                          child: const Text('İptal'),
                        ),
                        ElevatedButton(
                          onPressed: () => Navigator.of(context).pop(true),
                          child: const Text('Çıkış Yap'),
                        ),
                      ],
                    ),
                  );
                  if (confirm == true) {
                    await ref.read(authProvider.notifier).logout();
                  }
                },
                icon: const Icon(Icons.logout, color: Colors.red),
                label: const Text(
                  'Çıkış Yap',
                  style: TextStyle(color: Colors.red),
                ),
                style: OutlinedButton.styleFrom(
                  side: const BorderSide(color: Colors.red),
                  padding: const EdgeInsets.symmetric(vertical: 12),
                ),
              ),
            ),
          ],
        ),
      ),
    ),
  );
  }

  Widget _buildProfileItem(
    BuildContext context,
    IconData icon,
    String label,
    String value,
  ) {
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
      child: Row(
        children: [
          Icon(icon, color: Theme.of(context).colorScheme.primary, size: 24),
          const SizedBox(width: 16),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  label,
                  style: Theme.of(
                    context,
                  ).textTheme.bodySmall?.copyWith(color: Colors.grey[600]),
                ),
                const SizedBox(height: 2),
                Text(value, style: Theme.of(context).textTheme.bodyLarge),
              ],
            ),
          ),
        ],
      ),
    );
  }

  String _formatDate(DateTime date) {
    return '${date.day.toString().padLeft(2, '0')}.${date.month.toString().padLeft(2, '0')}.${date.year}';
  }

  String _formatBirthDate(String birthDate) {
    try {
      final date = DateTime.parse(birthDate);
      return _formatDate(date);
    } catch (e) {
      return birthDate;
    }
  }
}
