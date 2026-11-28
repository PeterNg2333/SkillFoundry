const flush = (queue: string[]) => {
  const seen: string[] = [];
  while (queue.length > 0) {
    const head = queue.shift();
    if (head !== undefined && !seen.includes(head)) seen.push(head);
  }
  return seen;
};
export { flush };
