import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';

class SettingsPage extends StatelessWidget {
  const SettingsPage({super.key});

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        leading: IconButton(
          icon: const Icon(Icons.arrow_back),
          onPressed: () => context.go('/'),
        ),
        title: const Text('Ayarlar'),
      ),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          _buildSection(
            context,
            title: 'Genel',
            children: [
              _buildSettingsTile(
                context,
                icon: Icons.language,
                title: 'Dil',
                subtitle: 'Türkçe',
                onTap: () {},
              ),
              _buildSettingsTile(
                context,
                icon: Icons.dark_mode_outlined,
                title: 'Tema',
                subtitle: 'Açık',
                onTap: () {},
              ),
            ],
          ),
          const SizedBox(height: 16),
          _buildSection(
            context,
            title: 'Bildirimler',
            children: [
              _buildSwitchTile(
                context,
                icon: Icons.notifications_outlined,
                title: 'Bildirimler',
                subtitle: 'Push bildirimleri al',
                value: true,
                onChanged: (val) {},
              ),
            ],
          ),
          const SizedBox(height: 16),
          _buildSection(
            context,
            title: 'Hakkında',
            children: [
              _buildSettingsTile(
                context,
                icon: Icons.info_outline,
                title: 'Uygulama Sürümü',
                subtitle: '1.0.0',
                onTap: () {},
              ),
              _buildSettingsTile(
                context,
                icon: Icons.description_outlined,
                title: 'Gizlilik Politikası',
                onTap: () {},
              ),
              _buildSettingsTile(
                context,
                icon: Icons.article_outlined,
                title: 'Kullanım Koşulları',
                onTap: () {},
              ),
            ],
          ),
        ],
      ),
    );
  }

  Widget _buildSection(
    BuildContext context, {
    required String title,
    required List<Widget> children,
  }) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Padding(
          padding: const EdgeInsets.only(left: 4, bottom: 8),
          child: Text(
            title,
            style: TextStyle(
              fontSize: 13,
              fontWeight: FontWeight.w600,
              color: Theme.of(context).colorScheme.primary,
              letterSpacing: 0.5,
            ),
          ),
        ),
        Card(
          margin: EdgeInsets.zero,
          child: Column(children: children),
        ),
      ],
    );
  }

  Widget _buildSettingsTile(
    BuildContext context, {
    required IconData icon,
    required String title,
    String? subtitle,
    required VoidCallback onTap,
  }) {
    return ListTile(
      leading: Icon(icon, color: Theme.of(context).colorScheme.primary),
      title: Text(title),
      subtitle: subtitle != null ? Text(subtitle) : null,
      trailing: const Icon(Icons.chevron_right, color: Colors.grey),
      onTap: onTap,
    );
  }

  Widget _buildSwitchTile(
    BuildContext context, {
    required IconData icon,
    required String title,
    String? subtitle,
    required bool value,
    required ValueChanged<bool> onChanged,
  }) {
    return SwitchListTile(
      secondary: Icon(icon, color: Theme.of(context).colorScheme.primary),
      title: Text(title),
      subtitle: subtitle != null ? Text(subtitle) : null,
      value: value,
      onChanged: onChanged,
    );
  }
}
