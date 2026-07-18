# OpenWorld Trip File
# Edit this file and run:  cargo run --bin travel -- examples/trip.md

trip:
  origin: BKK                     # Departure city or IATA code
  destination: TYO                # IATA city code (TYO, OSA, SIN, CDG, LHR…) or country name
  departure_date: "2026-08-01"  # YYYY-MM-DD
  return_date:    "2026-08-06"  # YYYY-MM-DD
  budget_max: "1500 USD"        # Total trip budget

flight:
  max_stops: 1                  # 0 = direct only, 1 = max one stop
  avoid_red_eye: true           # Skip overnight departures
  preferred_airlines:           # Ranked preference list
    - ANA
    - JAL
    - Singapore Airlines
    - Emirates

hotel:
  min_rating: 4.0               # Minimum star / review rating
  max_price_per_night: "150 USD"
  near_station: true            # Prefer hotels within walking distance of a station

transport:
  prefer_train: true            # Prefer rail over bus for inter-city
  avoid_overnight_bus: true

automation:
  auto_reserve: true            # Agent books automatically
  retry_on_failure: true        # Retry failed bookings with alternatives
  allow_replanning: true        # Re-plan if budget is exceeded

vault:
  max_single_transaction: "800 USD"   # Safety cap per transaction
