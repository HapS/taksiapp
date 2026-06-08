import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import 'services/ride_service.dart';

/// Geçmiş yolculuklar sayfası.
///
/// - Üstte filtre chip'leri: Tümü / Tamamlanan / İptal Edilen / Sürücü Bulunamadı
/// - Altta cursor-based infinite scroll ile liste
/// - Her kart: pickup → dropoff, mesafe/süre, ücret, karşı taraf, tarih, durum
///
/// Backend: GET /api/ride/history
class RideHistoryPage extends ConsumerStatefulWidget {
  const RideHistoryPage({super.key});

  @override
  ConsumerState<RideHistoryPage> createState() => _RideHistoryPageState();
}

class _RideHistoryPageState extends ConsumerState<RideHistoryPage> {
  final ScrollController _scrollController = ScrollController();

  static const int _pageSize = 20;

  /// Aktif status filtresi (null = hepsi)
  String? _statusFilter;

  /// Yüklenen tüm kayıtlar (sayfalar birleşik)
  final List<RideHistoryItem> _items = [];

  String? _nextCursor;
  bool _isLoading = false;
  bool _isLoadingMore = false;
  bool _hasMore = true;
  String? _error;
  String _resolvedRole = 'passenger';
  int _total = 0;

  @override
  void initState() {
    super.initState();
    _scrollController.addListener(_onScroll);
    _loadFirstPage();
  }

  @override
  void dispose() {
    _scrollController.removeListener(_onScroll);
    _scrollController.dispose();
    super.dispose();
  }

  void _onScroll() {
    if (!_hasMore || _isLoadingMore || _isLoading) return;
    if (_scrollController.position.pixels >=
        _scrollController.position.maxScrollExtent - 200) {
      _loadNextPage();
    }
  }

  Future<void> _loadFirstPage() async {
    setState(() {
      _isLoading = true;
      _error = null;
      _items.clear();
      _nextCursor = null;
      _hasMore = true;
    });
    await _fetch(reset: true);
  }

  Future<void> _loadNextPage() async {
    if (_isLoadingMore || !_hasMore || _nextCursor == null) return;
    setState(() => _isLoadingMore = true);
    await _fetch(reset: false);
  }

  Future<void> _fetch({required bool reset}) async {
    try {
      final result = await RideHistoryApi.getRideHistory(
        status: _statusFilter,
        role: 'auto',
        cursor: reset ? null : _nextCursor,
        limit: _pageSize,
      );
      if (!mounted) return;
      setState(() {
        _resolvedRole = result.role;
        _total = result.total;
        _hasMore = result.hasMore;
        _nextCursor = result.nextCursor;
        if (reset) {
          _items
            ..clear()
            ..addAll(result.rides);
        } else {
          _items.addAll(result.rides);
        }
        _isLoading = false;
        _isLoadingMore = false;
        _error = null;
      });
    } catch (e) {
      if (!mounted) return;
      setState(() {
        _isLoading = false;
        _isLoadingMore = false;
        _error = 'Yolculuklar yüklenirken hata oluştu';
      });
    }
  }

  void _onFilterChanged(String? status) {
    if (_statusFilter == status) return;
    setState(() => _statusFilter = status);
    _loadFirstPage();
  }

  Future<void> _onRefresh() async {
    await _loadFirstPage();
  }

  @override
  Widget build(BuildContext context) {
    final isDriverView = _resolvedRole == 'driver';

    return Scaffold(
      backgroundColor: Colors.grey[50],
      appBar: AppBar(
        backgroundColor: Theme.of(context).colorScheme.inversePrimary,
        elevation: 0,
        leading: IconButton(
          icon: const Icon(Icons.arrow_back),
          onPressed: () => context.canPop() ? context.pop() : context.go('/'),
        ),
        title: const Text(
          'Geçmiş Yolculuklar',
          style: TextStyle(fontWeight: FontWeight.bold),
        ),
      ),
      body: Column(
        children: [
          // Filtre bar — yatay scroll chip'leri
          _FilterBar(
            selected: _statusFilter,
            onChanged: _onFilterChanged,
          ),

          // Toplam kayıt sayısı
          if (!_isLoading && _error == null)
            Padding(
              padding: const EdgeInsets.fromLTRB(16, 0, 16, 8),
              child: Row(
                children: [
                  Icon(
                    isDriverView ? Icons.local_taxi : Icons.directions_car,
                    size: 16,
                    color: Colors.grey[600],
                  ),
                  const SizedBox(width: 6),
                  Text(
                    'Toplam $_total yolculuk',
                    style: TextStyle(
                      fontSize: 13,
                      color: Colors.grey[600],
                    ),
                  ),
                ],
              ),
            ),

          // İçerik
          Expanded(
            child: _buildBody(isDriverView),
          ),
        ],
      ),
    );
  }

  Widget _buildBody(bool isDriverView) {
    if (_isLoading && _items.isEmpty) {
      return const Center(child: CircularProgressIndicator());
    }

    if (_error != null && _items.isEmpty) {
      return Center(
        child: Padding(
          padding: const EdgeInsets.all(24.0),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              Icon(Icons.error_outline, size: 56, color: Colors.grey[400]),
              const SizedBox(height: 12),
              Text(
                _error!,
                style: TextStyle(color: Colors.grey[700], fontSize: 15),
                textAlign: TextAlign.center,
              ),
              const SizedBox(height: 16),
              ElevatedButton.icon(
                onPressed: _loadFirstPage,
                icon: const Icon(Icons.refresh),
                label: const Text('Tekrar Dene'),
              ),
            ],
          ),
        ),
      );
    }

    if (_items.isEmpty) {
      return Center(
        child: Padding(
          padding: const EdgeInsets.all(24.0),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              Icon(
                Icons.history,
                size: 64,
                color: Colors.grey[300],
              ),
              const SizedBox(height: 12),
              Text(
                'Henüz yolculuk geçmişin yok',
                style: TextStyle(
                  fontSize: 16,
                  color: Colors.grey[600],
                  fontWeight: FontWeight.w500,
                ),
              ),
              const SizedBox(height: 4),
              Text(
                _statusFilter == null
                    ? 'İlk yolculuğunu tamamladığında burada görünecek'
                    : 'Bu filtreye uygun yolculuk bulunamadı',
                style: TextStyle(fontSize: 13, color: Colors.grey[500]),
                textAlign: TextAlign.center,
              ),
            ],
          ),
        ),
      );
    }

    return RefreshIndicator(
      onRefresh: _onRefresh,
      child: ListView.separated(
        controller: _scrollController,
        physics: const AlwaysScrollableScrollPhysics(),
        padding: const EdgeInsets.fromLTRB(16, 8, 16, 24),
        itemCount: _items.length + (_hasMore ? 1 : 0),
        separatorBuilder: (_, _) => const SizedBox(height: 10),
        itemBuilder: (context, index) {
          if (index >= _items.length) {
            return const Padding(
              padding: EdgeInsets.symmetric(vertical: 16),
              child: Center(
                child: SizedBox(
                  width: 24,
                  height: 24,
                  child: CircularProgressIndicator(strokeWidth: 2.5),
                ),
              ),
            );
          }
          return _HistoryCard(
            item: _items[index],
            isDriverView: isDriverView,
          );
        },
      ),
    );
  }
}

// ---------------------------------------------------------------------------
// Filtre Bar
// ---------------------------------------------------------------------------

class _FilterBar extends StatelessWidget {
  final String? selected;
  final ValueChanged<String?> onChanged;

  const _FilterBar({required this.selected, required this.onChanged});

  @override
  Widget build(BuildContext context) {
    final primary = Theme.of(context).colorScheme.primary;
    final items = <_FilterItem>[
      _FilterItem(label: 'Tümü', value: null, icon: Icons.all_inclusive),
      _FilterItem(label: 'Tamamlanan', value: 'completed', icon: Icons.check_circle_outline),
      _FilterItem(label: 'İptal Edilen', value: 'cancelled', icon: Icons.cancel_outlined),
      _FilterItem(label: 'Sürücü Bulunamadı', value: 'no_driver', icon: Icons.search_off),
    ];

    return Container(
      width: double.infinity,
      color: Theme.of(context).colorScheme.inversePrimary,
      child: SingleChildScrollView(
        scrollDirection: Axis.horizontal,
        padding: const EdgeInsets.fromLTRB(16, 0, 16, 12),
        child: Row(
          children: [
            for (final item in items) ...[
              _Chip(
                label: item.label,
                icon: item.icon,
                isSelected: selected == item.value,
                color: primary,
                onTap: () => onChanged(item.value),
              ),
              const SizedBox(width: 8),
            ],
          ],
        ),
      ),
    );
  }
}

class _FilterItem {
  final String label;
  final String? value;
  final IconData icon;
  const _FilterItem({required this.label, required this.value, required this.icon});
}

class _Chip extends StatelessWidget {
  final String label;
  final IconData icon;
  final bool isSelected;
  final Color color;
  final VoidCallback onTap;

  const _Chip({
    required this.label,
    required this.icon,
    required this.isSelected,
    required this.color,
    required this.onTap,
  });

  @override
  Widget build(BuildContext context) {
    return InkWell(
      onTap: onTap,
      borderRadius: BorderRadius.circular(20),
      child: AnimatedContainer(
        duration: const Duration(milliseconds: 150),
        padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 8),
        decoration: BoxDecoration(
          color: isSelected ? color : Colors.white,
          borderRadius: BorderRadius.circular(20),
          border: Border.all(
            color: isSelected ? color : Colors.grey.shade300,
            width: 1,
          ),
        ),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(
              icon,
              size: 16,
              color: isSelected ? Colors.white : Colors.grey[700],
            ),
            const SizedBox(width: 6),
            Text(
              label,
              style: TextStyle(
                fontSize: 13,
                fontWeight: isSelected ? FontWeight.w600 : FontWeight.w500,
                color: isSelected ? Colors.white : Colors.grey[800],
              ),
            ),
          ],
        ),
      ),
    );
  }
}

// ---------------------------------------------------------------------------
// Yolculuk Kartı
// ---------------------------------------------------------------------------

class _HistoryCard extends StatelessWidget {
  final RideHistoryItem item;
  final bool isDriverView;

  const _HistoryCard({required this.item, required this.isDriverView});

  @override
  Widget build(BuildContext context) {
    final statusInfo = _statusInfo(item.status);
    final dateText = _formatDate(item.completedAt ?? item.cancelledAt ?? item.requestedAt);

    return Card(
      elevation: 0,
      margin: EdgeInsets.zero,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(14),
        side: BorderSide(color: Colors.grey.shade200),
      ),
      child: Padding(
        padding: const EdgeInsets.fromLTRB(14, 12, 14, 12),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            // Üst satır: tarih + durum chip
            Row(
              children: [
                Icon(Icons.calendar_today_outlined, size: 13, color: Colors.grey[500]),
                const SizedBox(width: 4),
                Text(
                  dateText,
                  style: TextStyle(fontSize: 12, color: Colors.grey[600]),
                ),
                const Spacer(),
                _StatusChip(info: statusInfo),
              ],
            ),

            const SizedBox(height: 10),
            const Divider(height: 1),
            const SizedBox(height: 10),

            // Pickup → Dropoff
            _RoutePoint(
              icon: Icons.radio_button_checked,
              iconColor: Colors.green,
              label: 'Alınma',
              address: item.pickupAddress,
            ),
            const SizedBox(height: 6),
            _RoutePoint(
              icon: Icons.location_on,
              iconColor: Colors.red,
              label: 'Varış',
              address: item.dropoffAddress,
            ),

            const SizedBox(height: 12),

            // Karşı taraf
            _CounterpartyRow(item: item, isDriverView: isDriverView),

            const SizedBox(height: 10),

            // Alt bilgi: mesafe + süre + ücret
            Row(
              children: [
                if (item.distanceKm != null) ...[
                  Icon(Icons.route_outlined, size: 14, color: Colors.grey[600]),
                  const SizedBox(width: 4),
                  Text(
                    _formatDistance(item.distanceKm!),
                    style: TextStyle(fontSize: 12, color: Colors.grey[700]),
                  ),
                  const SizedBox(width: 12),
                ],
                if (item.durationSec != null) ...[
                  Icon(Icons.access_time, size: 14, color: Colors.grey[600]),
                  const SizedBox(width: 4),
                  Text(
                    _formatDuration(item.durationSec!),
                    style: TextStyle(fontSize: 12, color: Colors.grey[700]),
                  ),
                  const SizedBox(width: 12),
                ],
                const Spacer(),
                if (item.fareAmount != null)
                  Text(
                    '${item.fareAmount!.toStringAsFixed(0)} ₺',
                    style: TextStyle(
                      fontSize: 16,
                      fontWeight: FontWeight.bold,
                      color: Theme.of(context).colorScheme.primary,
                    ),
                  ),
              ],
            ),
          ],
        ),
      ),
    );
  }
}

class _RoutePoint extends StatelessWidget {
  final IconData icon;
  final Color iconColor;
  final String label;
  final String address;

  const _RoutePoint({
    required this.icon,
    required this.iconColor,
    required this.label,
    required this.address,
  });

  @override
  Widget build(BuildContext context) {
    return Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Padding(
          padding: const EdgeInsets.only(top: 2),
          child: Icon(icon, size: 16, color: iconColor),
        ),
        const SizedBox(width: 8),
        Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                label,
                style: TextStyle(
                  fontSize: 11,
                  color: Colors.grey[500],
                  fontWeight: FontWeight.w500,
                ),
              ),
              const SizedBox(height: 1),
              Text(
                address,
                style: const TextStyle(fontSize: 13, fontWeight: FontWeight.w500),
                maxLines: 2,
                overflow: TextOverflow.ellipsis,
              ),
            ],
          ),
        ),
      ],
    );
  }
}

class _CounterpartyRow extends StatelessWidget {
  final RideHistoryItem item;
  final bool isDriverView;

  const _CounterpartyRow({required this.item, required this.isDriverView});

  @override
  Widget build(BuildContext context) {
    final cp = item.counterparty;
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 8),
      decoration: BoxDecoration(
        color: Colors.grey[50],
        borderRadius: BorderRadius.circular(8),
      ),
      child: Row(
        children: [
          CircleAvatar(
            radius: 16,
            backgroundColor: Theme.of(context).colorScheme.primary.withAlpha(30),
            child: Icon(
              isDriverView ? Icons.person_outline : Icons.local_taxi,
              size: 18,
              color: Theme.of(context).colorScheme.primary,
            ),
          ),
          const SizedBox(width: 10),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  cp.fullName,
                  style: const TextStyle(
                    fontSize: 13,
                    fontWeight: FontWeight.w600,
                  ),
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ),
                if (!isDriverView &&
                    (cp.vehicleModel != null || cp.vehiclePlate != null))
                  Text(
                    [
                      if (cp.vehicleModel != null) cp.vehicleModel!,
                      if (cp.vehiclePlate != null) cp.vehiclePlate!,
                    ].join(' • '),
                    style: TextStyle(fontSize: 11, color: Colors.grey[600]),
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                  ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}

class _StatusInfo {
  final String label;
  final Color color;
  final IconData icon;
  const _StatusInfo(this.label, this.color, this.icon);
}

_StatusInfo _statusInfo(String status) {
  switch (status) {
    case 'completed':
      return const _StatusInfo('Tamamlandı', Color(0xFF2E7D32), Icons.check_circle);
    case 'cancelled':
      return const _StatusInfo('İptal Edildi', Color(0xFFC62828), Icons.cancel);
    case 'no_driver':
      return const _StatusInfo('Sürücü Bulunamadı', Color(0xFFEF6C00), Icons.search_off);
    default:
      return _StatusInfo(status, Colors.grey, Icons.help_outline);
  }
}

class _StatusChip extends StatelessWidget {
  final _StatusInfo info;
  const _StatusChip({required this.info});

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
      decoration: BoxDecoration(
        color: info.color.withAlpha(25),
        borderRadius: BorderRadius.circular(12),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(info.icon, size: 12, color: info.color),
          const SizedBox(width: 4),
          Text(
            info.label,
            style: TextStyle(
              fontSize: 11,
              fontWeight: FontWeight.w600,
              color: info.color,
            ),
          ),
        ],
      ),
    );
  }
}

// ---------------------------------------------------------------------------
// Formatlama Yardımcıları
// ---------------------------------------------------------------------------

String _formatDistance(double km) {
  if (km < 1) {
    return '${(km * 1000).round()} m';
  }
  return '${km.toStringAsFixed(1)} km';
}

String _formatDuration(int sec) {
  if (sec < 60) {
    return '$sec sn';
  }
  final minutes = (sec / 60).round();
  if (minutes < 60) {
    return '~$minutes dk';
  }
  final hours = minutes ~/ 60;
  final rem = minutes % 60;
  return rem == 0 ? '${hours}S' : '${hours}S ${rem}dk';
}

String _formatDate(String iso) {
  try {
    final dt = DateTime.parse(iso).toLocal();
    String two(int n) => n.toString().padLeft(2, '0');
    const monthNames = [
      'Oca', 'Şub', 'Mar', 'Nis', 'May', 'Haz',
      'Tem', 'Ağu', 'Eyl', 'Eki', 'Kas', 'Ara',
    ];
    return '${dt.day.toString().padLeft(2, '0')} ${monthNames[dt.month - 1]} ${dt.year} • ${two(dt.hour)}:${two(dt.minute)}';
  } catch (_) {
    return iso;
  }
}
