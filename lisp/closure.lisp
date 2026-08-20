(defpackage #:closure
  (:use #:common-lisp))

(in-package #:closure)

(defun test-closure ()
  (let* ((x 3)
         (double (lambda (x)
                   (declare (type (signed-byte 32) x))
                   (* x 2))))
    (declare (type (signed-byte 32) x)
             (type function double))
    (assert (= 6 (funcall double x)))))

(defun test-capture ()
  (let* ((a 3)
         (add (lambda (x)
                (declare (type (signed-byte 32) x))
                (+ x a))))
    (declare (type (signed-byte 32) a)
             (type function add))
    (assert (= 6 (funcall add 3)))))

(test-closure)
(test-capture)
