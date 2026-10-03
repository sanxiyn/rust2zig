(defpackage #:operator
  (:use #:common-lisp)
  (:export #:not-bool #:not-int))

(in-package #:operator)

(declaim (ftype (function (boolean) boolean) not-bool))
(defun not-bool (b)
  (not b))

(declaim (ftype (function ((unsigned-byte 8)) (unsigned-byte 8)) not-int))
(defun not-int (x)
  (ldb (byte 8 0) (lognot x)))

(defun test-not ()
  (assert (equal nil (not-bool t)))
  (assert (equal t (not-bool nil)))
  (assert (= 240 (not-int 15)))
  (assert (= 255 (not-int 0))))

(test-not)
