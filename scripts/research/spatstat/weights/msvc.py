def med3(a, f, m, l, lt):
    if lt(a[m], a[f]): a[m], a[f] = a[f], a[m]
    if lt(a[l], a[m]):
        a[l], a[m] = a[m], a[l]
        if lt(a[m], a[f]): a[m], a[f] = a[f], a[m]
def guess_median(a, f, m, l, lt):
    count = l - f
    if 40 < count:
        step = (count + 1) >> 3; two = step << 1
        med3(a, f, f + step, f + two, lt); med3(a, m - step, m, m + step, lt)
        med3(a, l - two, l - step, l, lt); med3(a, f + step, m, l - step, lt)
    else:
        med3(a, f, m, l, lt)
def partition(a, first, last, lt):
    mid = first + ((last - first) >> 1)
    guess_median(a, first, mid, last - 1, lt)
    pf = mid; pl = pf + 1
    while first < pf and not lt(a[pf - 1], a[pf]) and not lt(a[pf], a[pf - 1]): pf -= 1
    while pl < last and not lt(a[pl], a[pf]) and not lt(a[pf], a[pl]): pl += 1
    gf = pl; gl = pf
    while True:
        while gf < last:
            if lt(a[pf], a[gf]): pass
            elif lt(a[gf], a[pf]): break
            elif pl != gf:
                a[pl], a[gf] = a[gf], a[pl]; pl += 1
            else: pl += 1
            gf += 1
        while first < gl:
            p = gl - 1
            if lt(a[p], a[pf]): pass
            elif lt(a[pf], a[p]): break
            else:
                pf -= 1
                if pf != p: a[pf], a[p] = a[p], a[pf]
            gl -= 1
        if gl == first and gf == last: return pf, pl
        if gl == first:
            if pl != gf: a[pf], a[pl] = a[pl], a[pf]
            pl += 1
            a[pf], a[gf] = a[gf], a[pf]
            pf += 1; gf += 1
        elif gf == last:
            gl -= 1; pf -= 1
            if gl != pf: a[gl], a[pf] = a[pf], a[gl]
            pl -= 1
            a[pf], a[pl] = a[pl], a[pf]
        else:
            gl -= 1
            a[gf], a[gl] = a[gl], a[gf]
            gf += 1
def insertion_sort(a, first, last, lt):
    for mid in range(first + 1, last):
        val = a[mid]
        if lt(val, a[first]):
            a[first + 1:mid + 1] = a[first:mid]; a[first] = val
        else:
            hole = mid
            while lt(val, a[hole - 1]):
                a[hole] = a[hole - 1]; hole -= 1
            a[hole] = val
def nth_element(a, first, nth, last, lt, isort_max=32):
    if nth == last: return
    while isort_max < last - first:
        pf, pl = partition(a, first, last, lt)
        if pl <= nth: first = pl
        elif pf <= nth: return
        else: last = pf
    insertion_sort(a, first, last, lt)
