#ifndef _ABIBITS_SIGVAL_H
#define _ABIBITS_SIGVAL_H

union sigval {
    int   sival_int;
    void *sival_ptr;
};

#endif
