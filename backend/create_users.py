import requests
import json
import random

ENDPOINT = "https://one.web.tr/api/auth/register"
PASSWORD = "123456ck"

first_names = [
    "Ahmet", "Mehmet", "Ali", "Veli", "Can", "Efe", "Deniz", "Emre", "Onur", "Kaan",
    "Burak", "Cem", "Okan", "Serkan", "Hakan", "Murat", "Zafer", "Umut", "Tolga", "Barış",
    "Ayşe", "Fatma", "Zeynep", "Elif", "Merve", "Sena", "İrem", "Dilara", "Cansu", "Büşra",
    "Esra", "Gamze", "Aslı", "Çağla", "Melis", "Ebru", "Sibel", "Pelin", "Hande", "Yeliz",
    "John", "Emma", "Liam", "Sophia", "Noah", "Olivia", "James", "Ava", "Oliver", "Mia",
    "Lucas", "Isabella", "Mason", "Charlotte", "Ethan", "Amelia", "Logan", "Harper", "Alexander", "Evelyn"
]

last_names = [
    "Yılmaz", "Demir", "Çelik", "Kaya", "Öztürk", "Koç", "Şahin", "Doğan", "Yıldız", "Arslan",
    "Aydın", "Kurt", "Aslan", "Çiçek", "Aksoy", "Aktaş", "Kara", "Özkan", "Bulut", "Erdoğan",
    "Smith", "Johnson", "Williams", "Brown", "Jones", "Garcia", "Miller", "Davis", "Wilson", "Moore",
    "Taylor", "Anderson", "Thomas", "Jackson", "White", "Harris", "Martin", "Thompson", "Martinez", "Robinson"
]

used_emails = set()

def make_user(i):
    first = random.choice(first_names)
    last = random.choice(last_names)

    email_base = f"{first.lower()}.{last.lower()}".replace("ç", "c").replace("ğ", "g").replace("ı", "i").replace("ö", "o").replace("ş", "s").replace("ü", "u")

    email = f"{email_base}.{i}@one.web.tr"
    if email in used_emails:
        email = f"{email_base}{i}@one.web.tr"
    used_emails.add(email)

    username = f"{first.lower()}.{last.lower()}.{i}".replace("ç", "c").replace("ğ", "g").replace("ı", "i").replace("ö", "o").replace("ş", "s").replace("ü", "u")

    return {
        "username": username,
        "email": email,
        "password": PASSWORD,
        "first_name": first,
        "last_name": last,
    }

success = 0
fail = 0

for i in range(1, 101):
    user = make_user(i)
    try:
        r = requests.post(ENDPOINT, json=user, timeout=10)
        if r.status_code in (200, 201):
            success += 1
            print(f"[OK] {i:3d} {user['email']} → {r.status_code}")
        else:
            fail += 1
            print(f"[FAIL] {i:3d} {user['email']} → {r.status_code} {r.text[:120]}")
    except Exception as e:
        fail += 1
        print(f"[ERROR] {i:3d} {user['email']} → {e}")

print(f"\nDone: {success} success, {fail} failed")
