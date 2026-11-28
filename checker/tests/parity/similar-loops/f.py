def sums(entries):
    total = 0
    for entry in entries:
        cost = entry.price * entry.quantity
        total += cost - entry.discount
    return total
