def totals(orders):
    total = 0
    for order in orders:
        price = order.price * order.quantity
        total += price - order.discount
    return total
