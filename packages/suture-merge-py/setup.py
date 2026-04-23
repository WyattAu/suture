from setuptools import setup, find_packages

setup(
    name="suture-merge-driver",
    version="5.0.0",
    description="Git merge driver that semantically merges JSON, YAML, TOML, CSV, XML, Markdown, DOCX, XLSX, PPTX, and more",
    long_description=open("README.md").read(),
    long_description_content_type="text/markdown",
    author="WyattAu",
    url="https://github.com/WyattAu/suture",
    license="Apache-2.0",
    packages=find_packages(),
    python_requires=">=3.8",
    entry_points={
        "console_scripts": [
            "suture-merge-driver=suture_merge_driver.cli:main",
        ],
    },
    classifiers=[
        "Development Status :: 5 - Production/Stable",
        "License :: OSI Approved :: Apache Software License",
        "Programming Language :: Python :: 3",
        "Topic :: Software Development :: Version Control",
    ],
)
