const sum = (items) => {
  let total = 0;
  for (const item of items) {
    const price = item.price * item.quantity;
    total += price - item.discount;
  }
  return total;
};
export { sum };
