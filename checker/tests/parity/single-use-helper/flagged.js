const helper = () => 1;
const shared = () => 2;
const run = () => helper() + shared() + shared();
export { run };
