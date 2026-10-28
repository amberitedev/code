-- Icons are served by their URL, so finding one must not read every shared instance.
CREATE INDEX shared_instances_icon ON shared_instances(icon);
