const add = (rows) => {
  let total = 0;
  for (const row of rows) {
    const cost = row.price * row.quantity;
    total += cost - row.discount;
  }
  return total;
};
export { add };
