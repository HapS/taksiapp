# Flutter Map Camera Fit Guide

## Issue: Map Camera Focus Lost on Page Navigation (Rebuild)

When navigating away from the Map page (e.g., to `/profile` via `GoRouter`) and returning, the map widget is reconstructed. 
Although the route points/destination state is preserved (e.g., via a Riverpod notifier), calling `_mapController.fitCamera(...)` directly inside the `onMapReady` callback of `FlutterMap` fails to center/zoom the map to the route.

### Root Cause
At the moment `onMapReady` is triggered during the widget's build/layout sequence, the map container's layout size (width/height) might not yet be fully calculated (size is `0x0`). Therefore, any camera movement calculations that depend on viewport size (like `fitCamera`) fail or are ignored.

### Solution
Wrap any camera centering/bounds-fitting logic inside `WidgetsBinding.instance.addPostFrameCallback`. This ensures the camera fits only after the map widget has completed its first layout pass and has a non-zero viewport size.

```dart
options: MapOptions(
  initialCenter: rideState.currentLocation ?? _defaultCenter,
  initialZoom: 14,
  onTap: _onMapTap,
  onMapReady: () {
    if (rideState.routePoints.isNotEmpty && rideState.destination != null) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted) {
          final bounds = LatLngBounds.fromPoints(rideState.routePoints);
          _mapController.fitCamera(
            CameraFit.bounds(
              bounds: bounds,
              padding: const EdgeInsets.all(50),
            ),
          );
        }
      });
    }
  },
),
```
