def _helper():
    return 1


def _shared():
    return 2


def public_entry():
    return _helper() + _shared() + _shared()


class Store:
    def _load(self):
        return 1

    def run(self):
        return self._load()
