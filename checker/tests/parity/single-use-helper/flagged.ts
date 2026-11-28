const helper = (): number => 1;
const shared = (): number => 2;
const run = () => helper() + shared() + shared();
export { run };
