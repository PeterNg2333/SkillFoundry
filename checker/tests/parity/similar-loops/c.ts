const drain = (queue: string[]) => {
  const seen: string[] = [];
  while (queue.length > 0) {
    const next = queue.shift();
    if (next !== undefined && !seen.includes(next)) seen.push(next);
  }
  return seen;
};
export { drain };
