#!/usr/bin/env python3
"""
Fake Sürücü Botu — JWT Auth versiyonu
- Backend'e login olur, JWT access token alır
- WS bağlantısını token ile kurar: /ws/driver?token=<jwt>
- Gelen teklifleri kabul eder, pickup → picked_up → dropoff → completed akışını yapar
- Bağlantı koparsa yeniden login edip bağlanır
- Yarım kalan ride durumunu kontrol eder (auto-complete YOK, state'ten devam)
"""

import asyncio
import json
import math
import signal
import sys
import urllib.error
import urllib.parse
import urllib.request
from datetime import datetime

import websockets

# --- Ayarlar ---
API_BASE_URL = "https://one.web.tr/api"
WS_BASE_URL = "wss://one.web.tr"

# (username, password, başlangıç_lat, başlangıç_lon)
DRIVERS = [
    ("deniz.moore.1", "123456ck", 40.678537, 30.251390),
    ("sibel.ozturk.2", "123456ck", 40.681055, 30.282320),
    ("isabella.dogan.3", "123456ck", 40.679982, 30.316398),
    ("tolga.celik.4", "123456ck", 40.680606, 30.351055),
    ("elif.jackson.5", "123456ck", 40.678375, 30.381313),
    ("kaan.thompson.6", "123456ck", 40.681343, 30.416231),
    ("merve.johnson.7", "123456ck", 40.681049, 30.447808),
    ("elif.jones.8", "123456ck", 40.679782, 30.483986),
    ("tolga.arslan.9", "123456ck", 40.678915, 30.518181),
    ("umut.kara.10", "123456ck", 40.681606, 30.547822),
    ("melis.thompson.11", "123456ck", 40.700302, 30.250166),
    ("murat.sahin.12", "123456ck", 40.703957, 30.282825),
    ("liam.thomas.13", "123456ck", 40.701066, 30.316288),
    ("logan.yildiz.14", "123456ck", 40.700316, 30.348787),
    ("can.martin.15", "123456ck", 40.701952, 30.383183),
    ("hakan.anderson.16", "123456ck", 40.701132, 30.415423),
    ("yeliz.aydin.17", "123456ck", 40.701075, 30.449638),
    ("liam.thomas.18", "123456ck", 40.701359, 30.481186),
    ("lucas.miller.19", "123456ck", 40.703550, 30.516626),
    ("oliver.harris.20", "123456ck", 40.702769, 30.548444),
    ("isabella.harris.21", "123456ck", 40.726370, 30.251440),
    ("alexander.thomas.22", "123456ck", 40.722884, 30.282631),
    ("charlotte.erdogan.23", "123456ck", 40.725286, 30.317445),
    ("serkan.celik.24", "123456ck", 40.726146, 30.349588),
    ("amelia.ozkan.25", "123456ck", 40.725720, 30.383881),
    ("oliver.koc.26", "123456ck", 40.723613, 30.416850),
    ("liam.davis.27", "123456ck", 40.725930, 30.451185),
    ("pelin.williams.28", "123456ck", 40.724421, 30.483456),
    ("noah.wilson.29", "123456ck", 40.722538, 30.515371),
    ("noah.martinez.30", "123456ck", 40.725590, 30.549357),
    ("mehmet.celik.31", "123456ck", 40.745292, 30.250195),
    ("asli.martinez.32", "123456ck", 40.747412, 30.283998),
    ("hande.davis.33", "123456ck", 40.746099, 30.316356),
    ("efe.dogan.34", "123456ck", 40.746634, 30.351014),
    ("liam.martin.35", "123456ck", 40.746684, 30.382773),
    ("zafer.koc.36", "123456ck", 40.746559, 30.414618),
    ("can.martinez.37", "123456ck", 40.744774, 30.450614),
    ("amelia.arslan.38", "123456ck", 40.748533, 30.483473),
    ("fatma.bulut.39", "123456ck", 40.746174, 30.515081),
    ("mia.aslan.40", "123456ck", 40.746609, 30.551628),
    ("ahmet.yildiz.41", "123456ck", 40.769882, 30.250158),
    ("john.aktas.42", "123456ck", 40.770241, 30.282229),
    ("emre.yilmaz.43", "123456ck", 40.768855, 30.318410),
    ("cem.aksoy.44", "123456ck", 40.769111, 30.349737),
    ("emre.arslan.45", "123456ck", 40.767877, 30.383392),
    ("veli.smith.47", "123456ck", 40.770628, 30.414523),
    ("noah.koc.48", "123456ck", 40.769935, 30.451082),
    ("evelyn.martin.49", "123456ck", 40.770345, 30.484062),
    ("emre.yilmaz.50", "123456ck", 40.770037, 30.516475),
    ("sibel.davis.51", "123456ck", 40.769045, 30.549404),
    ("zafer.brown.52", "123456ck", 40.789224, 30.251480),
    ("ahmet.koc.53", "123456ck", 40.791280, 30.282099),
    ("serkan.anderson.54", "123456ck", 40.791019, 30.316540),
    ("yeliz.kara.55", "123456ck", 40.790427, 30.349284),
    ("oliver.dogan.56", "123456ck", 40.791154, 30.383694),
    ("ebru.white.57", "123456ck", 40.791450, 30.416333),
    ("umut.celik.58", "123456ck", 40.789112, 30.448718),
    ("okan.dogan.59", "123456ck", 40.789709, 30.483438),
    ("kaan.dogan.60", "123456ck", 40.792444, 30.517594),
    ("sophia.harris.61", "123456ck", 40.792188, 30.550966),
    ("yeliz.sahin.62", "123456ck", 40.812221, 30.251367),
    ("gamze.bulut.63", "123456ck", 40.813892, 30.281633),
    ("umut.garcia.64", "123456ck", 40.811267, 30.314658),
    ("noah.white.65", "123456ck", 40.814222, 30.348898),
    ("hakan.aslan.66", "123456ck", 40.811638, 30.383699),
    ("serkan.smith.67", "123456ck", 40.812578, 30.414778),
    ("olivia.aktas.68", "123456ck", 40.811839, 30.449910),
    ("merve.williams.69", "123456ck", 40.811873, 30.482192),
    ("elif.sahin.70", "123456ck", 40.814046, 30.516219),
    ("ebru.yilmaz.71", "123456ck", 40.812488, 30.549595),
    ("asli.yildiz.72", "123456ck", 40.833495, 30.249546),
    ("baris.williams.73", "123456ck", 40.835084, 30.282052),
    ("hande.davis.74", "123456ck", 40.833835, 30.318199),
    ("ava.white.75", "123456ck", 40.835440, 30.348736),
    ("melis.aktas.76", "123456ck", 40.835823, 30.384468),
    ("busra.ozturk.77", "123456ck", 40.833483, 30.414571),
    ("sophia.ozturk.78", "123456ck", 40.833986, 30.450675),
    ("cagla.arslan.79", "123456ck", 40.834041, 30.483918),
    ("onur.kara.80", "123456ck", 40.836113, 30.516579),
    ("mehmet.yildiz.81", "123456ck", 40.834282, 30.551602),
    ("asli.yildiz.82", "123456ck", 40.858791, 30.250066),
    ("mia.ozkan.83", "123456ck", 40.856493, 30.283894),
    ("melis.garcia.84", "123456ck", 40.857180, 30.316903),
    ("esra.garcia.85", "123456ck", 40.856885, 30.350424),
    ("veli.celik.86", "123456ck", 40.855835, 30.382394),
    ("oliver.jones.87", "123456ck", 40.859472, 30.418002),
    ("mehmet.ozkan.88", "123456ck", 40.856826, 30.451234),
    ("ayse.white.89", "123456ck", 40.856841, 30.484857),
    ("zeynep.miller.90", "123456ck", 40.858575, 30.516065),
    ("sophia.sahin.91", "123456ck", 40.856609, 30.547734),
    ("cagla.yilmaz.92", "123456ck", 40.881315, 30.248152),
    ("esra.johnson.93", "123456ck", 40.881078, 30.285149),
    ("elif.taylor.94", "123456ck", 40.880081, 30.315286),
    ("noah.aslan.96", "123456ck", 40.881271, 30.351795),
    ("tolga.harris.97", "123456ck", 40.880616, 30.383235),
    ("charlotte.davis.98", "123456ck", 40.879312, 30.415888),
    ("cagla.martinez.99", "123456ck", 40.878623, 30.450497),
    ("elif.anderson.100", "123456ck", 40.879532, 30.481876),
]

LOCATION_INTERVAL = 2
PING_INTERVAL = 4
ACCEPT_DELAY = 2
ARRIVAL_HOLD_SEC = 3
STEP = 0.002  # ~200m/tick sabit hız
# ----------------


def log(msg: str):
    ts = datetime.now().strftime("%H:%M:%S.%f")[:-3]
    print(f"[{ts}] [BOT] {msg}", flush=True)


def move_toward(clat, clon, tlat, tlon, step=STEP):
    dlat = tlat - clat
    dlon = tlon - clon
    dist = math.sqrt(dlat**2 + dlon**2)
    if dist < step:
        return tlat, tlon, True
    ratio = step / dist
    return clat + dlat * ratio, clon + dlon * ratio, False


def http_post(url: str, body: dict, token: str = None) -> tuple[bool, int, str]:
    data = json.dumps(body).encode()
    headers = {"Content-Type": "application/json"}
    if token:
        headers["Authorization"] = f"Bearer {token}"
    req = urllib.request.Request(url, data=data, headers=headers, method="POST")
    try:
        with urllib.request.urlopen(req, timeout=10) as resp:
            return True, resp.status, resp.read().decode()
    except urllib.error.HTTPError as e:
        return False, e.code, e.read().decode(errors="replace")
    except urllib.error.URLError as e:
        return False, 0, str(e)


def http_get(url: str, token: str = None) -> tuple[bool, int, str]:
    headers = {"Accept": "application/json"}
    if token:
        headers["Authorization"] = f"Bearer {token}"
    req = urllib.request.Request(url, headers=headers)
    try:
        with urllib.request.urlopen(req, timeout=10) as resp:
            return True, resp.status, resp.read().decode()
    except urllib.error.HTTPError as e:
        return False, e.code, e.read().decode(errors="replace")
    except urllib.error.URLError as e:
        return False, 0, str(e)


async def login(username: str, password: str) -> str | None:
    url = f"{API_BASE_URL}/auth/login"
    ok, code, body = await asyncio.to_thread(
        http_post, url, {"username": username, "password": password}
    )
    if not ok:
        log(f"Login başarısız ({username}): HTTP {code} — {body[:200]}")
        return None
    try:
        data = json.loads(body)
        token = data.get("tokens", {}).get("access_token") or data.get("access_token")
        if token:
            log(f"Login başarılı: {username}")
            return token
        log(f"Login yanıtında token yok ({username}): {body[:200]}")
        return None
    except Exception as e:
        log(f"Login parse hatası ({username}): {e}")
        return None


async def update_ride_status(ride_id: int, status: str, token: str) -> bool:
    url = f"{API_BASE_URL}/ride/{ride_id}/status"
    ok, code, body = await asyncio.to_thread(http_post, url, {"status": status}, token)
    log(
        f"  → ride #{ride_id} status={status} HTTP {code} {'✅' if ok else '❌'}: {body[:200]}"
    )
    return ok


async def cancel_ride(ride_id: int, token: str) -> bool:
    url = f"{API_BASE_URL}/ride/{ride_id}/cancel"
    ok, code, body = await asyncio.to_thread(http_post, url, {"by": "driver"}, token)
    log(f"  → ride #{ride_id} cancel HTTP {code} {'✅' if ok else '❌'}: {body[:200]}")
    return ok


async def fetch_ride_coords(
    ride_id: int, token: str
) -> tuple[float | None, float | None]:
    url = f"{API_BASE_URL}/ride/{ride_id}"
    ok, code, body = await asyncio.to_thread(http_get, url, token)
    if not ok:
        return None, None
    try:
        data = json.loads(body)
        return data.get("dropoff_lat"), data.get("dropoff_lon")
    except Exception:
        return None, None


async def check_driver_active_ride(token: str, username: str, state: dict):
    """Fresh start'ta backend'den aktif ride kontrol et.
    Varsa tamamla → sürücü yeni teklif alabilir."""
    url = f"{API_BASE_URL}/ride/driver/active"
    ok, code, body = await asyncio.to_thread(http_get, url, token)
    if not ok:
        return
    try:
        data = json.loads(body)
        ride = data.get("active_ride")
        if ride is None:
            log(f"[{username}] Aktif ride yok, müsait")
            return
        ride_id = ride.get("ride_id")
        status = ride.get("status", "")
        log(f"[{username}] Yarım kalan ride #{ride_id} ({status})")
        if status in ("accepted", "picked_up"):
            completed = await update_ride_status(ride_id, "completed", token)
            if completed:
                log(f"[{username}] Ride #{ride_id} tamamlandı (yarım kalan)")
            else:
                await cancel_ride(ride_id, token)
                log(f"[{username}] Ride #{ride_id} iptal edildi (tamamlanamadı)")
    except Exception as e:
        log(f"[{username}] Aktif ride kontrol hatası: {e}")


async def run_bot(username: str, token: str, state: dict):
    ws_url = f"{WS_BASE_URL}/ws/driver?token={urllib.parse.quote(token)}"
    lat = state["lat"]
    lon = state["lon"]
    target_lat = state.get("target_lat")
    target_lon = state.get("target_lon")
    phase = state.get("phase", "idle")
    active_ride_id = state.get("active_ride_id")
    dropoff_lat = state.get("dropoff_lat")
    dropoff_lon = state.get("dropoff_lon")

    # State'te aktif ride varsa: backend'den durum kontrol et
    if active_ride_id is not None and phase not in ("idle", "arrived"):
        ok, code, body = await asyncio.to_thread(
            http_get, f"{API_BASE_URL}/ride/{active_ride_id}", token
        )
        if ok:
            try:
                data = json.loads(body)
                s = data.get("status", "")
                if s in ("completed", "cancelled", "no_driver"):
                    log(
                        f"[{username}] Ride #{active_ride_id} durumu '{s}' — idle'a dönülüyor."
                    )
                    phase = "idle"
                    active_ride_id = None
                    target_lat = target_lon = None
                    dropoff_lat = dropoff_lon = None
                    state.update(
                        {
                            "phase": "idle",
                            "active_ride_id": None,
                            "target_lat": None,
                            "target_lon": None,
                            "dropoff_lat": None,
                            "dropoff_lon": None,
                        }
                    )
                else:
                    log(
                        f"[{username}] Ride #{active_ride_id} durumu '{s}' — state'ten devam"
                    )
            except json.JSONDecodeError:
                pass
    else:
        # Fresh start: backend'den aktif ride kontrol et
        await check_driver_active_ride(token, username, state)

    log(f"[{username}] Bağlanılıyor: {ws_url[:60]}...")

    async with websockets.connect(ws_url) as ws:
        log(f"[{username}] Bağlantı kuruldu. Teklif bekleniyor...")

        async def send(msg: dict):
            await ws.send(json.dumps(msg))

        async def location_loop():
            nonlocal lat, lon, target_lat, target_lon, phase
            nonlocal active_ride_id, dropoff_lat, dropoff_lon

            while True:
                await asyncio.sleep(LOCATION_INTERVAL)

                if target_lat is not None:
                    lat, lon, arrived = move_toward(lat, lon, target_lat, target_lon)

                    if arrived:
                        log(f"[{username}] Hedefe ulaşıldı (phase={phase})")
                        lat, lon = target_lat, target_lon
                        target_lat = None
                        target_lon = None
                        state["target_lat"] = None
                        state["target_lon"] = None

                        if phase == "driving_to_pickup" and active_ride_id:
                            await asyncio.sleep(ARRIVAL_HOLD_SEC)
                            ok = await update_ride_status(
                                active_ride_id, "picked_up", token
                            )
                            if ok:
                                phase = "picked_up"
                                state["phase"] = phase
                                dlat, dlon = dropoff_lat, dropoff_lon
                                if dlat is None:
                                    log(
                                        f"[{username}] dropoff_lat None, backend'den çekiliyor..."
                                    )
                                    dlat, dlon = await fetch_ride_coords(
                                        active_ride_id, token
                                    )
                                    state["dropoff_lat"] = dlat
                                    state["dropoff_lon"] = dlon
                                if dlat is not None:
                                    target_lat = dlat
                                    target_lon = dlon
                                    phase = "driving_to_dropoff"
                                    state["phase"] = phase
                                    state["target_lat"] = target_lat
                                    state["target_lon"] = target_lon
                                    log(
                                        f"[{username}] Dropoff'a yürünüyor: ({target_lat:.4f}, {target_lon:.4f})"
                                    )
                                else:
                                    log(
                                        f"[{username}] ⚠️ dropoff koordinatı bulunamadı, bekleniyor..."
                                    )
                            else:
                                log(
                                    f"[{username}] ⚠️ picked_up başarısız — idle'a geçiliyor"
                                )
                                phase = "idle"
                                active_ride_id = None
                                dropoff_lat = dropoff_lon = None
                                target_lat = target_lon = None
                                state.update(
                                    {
                                        "phase": "idle",
                                        "active_ride_id": None,
                                        "dropoff_lat": None,
                                        "dropoff_lon": None,
                                        "target_lat": None,
                                        "target_lon": None,
                                    }
                                )

                        elif phase == "driving_to_dropoff" and active_ride_id:
                            await asyncio.sleep(ARRIVAL_HOLD_SEC)
                            log(
                                f"[{username}] ✅ Varış noktasına ulaşıldı, tamamlanıyor..."
                            )
                            rid = active_ride_id
                            ok = await update_ride_status(rid, "completed", token)
                            if ok:
                                log(f"[{username}] 🎉 Yolculuk tamamlandı!")
                                phase = "idle"
                                active_ride_id = None
                                dropoff_lat = dropoff_lon = None
                                state.update(
                                    {
                                        "phase": "idle",
                                        "active_ride_id": None,
                                        "dropoff_lat": None,
                                        "dropoff_lon": None,
                                        "target_lat": None,
                                        "target_lon": None,
                                    }
                                )
                            else:
                                log(
                                    f"[{username}] ⚠️ completed başarısız — idle'a geçiliyor"
                                )
                                phase = "idle"
                                active_ride_id = None
                                dropoff_lat = dropoff_lon = None
                                target_lat = target_lon = None
                                state.update(
                                    {
                                        "phase": "idle",
                                        "active_ride_id": None,
                                        "dropoff_lat": None,
                                        "dropoff_lon": None,
                                        "target_lat": None,
                                        "target_lon": None,
                                    }
                                )

                state["lat"] = lat
                state["lon"] = lon

                await send({"type": "location_update", "lat": lat, "lon": lon})

        async def ping_loop():
            while True:
                await asyncio.sleep(PING_INTERVAL)
                await send({"type": "ping"})

        async def message_loop():
            nonlocal target_lat, target_lon, phase, active_ride_id
            nonlocal dropoff_lat, dropoff_lon

            async for raw in ws:
                try:
                    msg = json.loads(raw)
                except json.JSONDecodeError:
                    continue

                mtype = msg.get("type")

                if mtype == "ride_offer":
                    ride_id = msg["ride_id"]
                    active_ride_id = ride_id
                    pickup_lat = msg.get("pickup_lat")
                    pickup_lon = msg.get("pickup_lon")
                    dropoff_lat = msg.get("dropoff_lat")
                    dropoff_lon = msg.get("dropoff_lon")
                    state["active_ride_id"] = active_ride_id
                    state["dropoff_lat"] = dropoff_lat
                    state["dropoff_lon"] = dropoff_lon
                    log(
                        f"[{username}] Teklif! ride #{ride_id} | "
                        f"{msg.get('pickup_address')} → {msg.get('dropoff_address')}"
                    )

                    await asyncio.sleep(ACCEPT_DELAY)
                    await send(
                        {"type": "offer_response", "ride_id": ride_id, "accepted": True}
                    )
                    log(f"[{username}] Kabul edildi ✅ ride #{ride_id}")

                    target_lat = pickup_lat
                    target_lon = pickup_lon
                    phase = "driving_to_pickup"
                    state["phase"] = phase
                    state["target_lat"] = target_lat
                    state["target_lon"] = target_lon
                    log(
                        f"[{username}] Pickup'a yürünüyor: ({target_lat:.4f}, {target_lon:.4f})"
                    )

                elif mtype == "ride_status_changed":
                    status = msg.get("status")
                    log(f"[{username}] Ride durumu: {status}")
                    if status in ("completed", "cancelled", "no_driver"):
                        target_lat = target_lon = None
                        phase = "idle"
                        active_ride_id = None
                        dropoff_lat = dropoff_lon = None
                        state.update(
                            {
                                "phase": "idle",
                                "active_ride_id": None,
                                "dropoff_lat": None,
                                "dropoff_lon": None,
                                "target_lat": None,
                                "target_lon": None,
                            }
                        )
                        log(f"[{username}] Müsait, yeni teklif bekleniyor...")

                elif mtype == "pong":
                    pass

                elif mtype == "error":
                    log(f"[{username}] Sunucu hatası: {msg.get('message')}")

        await asyncio.gather(location_loop(), ping_loop(), message_loop())


async def driver_loop(username: str, password: str, start_lat: float, start_lon: float):
    retry_delay = 5
    state = {
        "lat": start_lat,
        "lon": start_lon,
        "target_lat": None,
        "target_lon": None,
        "phase": "idle",
        "active_ride_id": None,
        "dropoff_lat": None,
        "dropoff_lon": None,
    }

    while True:
        token = await login(username, password)
        if not token:
            log(f"[{username}] Login başarısız. {retry_delay}sn sonra tekrar...")
            await asyncio.sleep(retry_delay)
            continue

        try:
            await run_bot(username, token, state)
        except websockets.exceptions.ConnectionClosedError as e:
            log(f"[{username}] Bağlantı kesildi: {e}. {retry_delay}sn sonra yeniden...")
        except websockets.exceptions.InvalidStatusCode as e:
            log(
                f"[{username}] WS bağlantı hatası (HTTP {e.status_code}) — token süresi dolmuş olabilir. Yeniden login..."
            )
        except OSError as e:
            log(f"[{username}] Bağlantı hatası: {e}. {retry_delay}sn sonra...")
        except Exception as e:
            log(f"[{username}] Beklenmedik hata: {e}. {retry_delay}sn sonra...")

        await asyncio.sleep(retry_delay)


async def main():
    await asyncio.gather(
        *[
            driver_loop(username, password, lat, lon)
            for username, password, lat, lon in DRIVERS
        ]
    )


if __name__ == "__main__":

    def handle_exit(sig, frame):
        log("Bot durduruluyor...")
        sys.exit(0)

    signal.signal(signal.SIGINT, handle_exit)
    signal.signal(signal.SIGTERM, handle_exit)

    asyncio.run(main())
